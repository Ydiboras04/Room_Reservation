//! Handlers : extraire la requête, appeler le use case, convertir le résultat.
//! Aucune règle métier ici. Chaque handler qui modifie des réservations prend le verrou
//! d'écriture (voir `AppState::lock_writes`).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use uuid::Uuid;

use crate::{
    application::{
        errors::ApplicationError,
        ports::RoomRepository,
        use_cases::{
            cancel_booking::cancel_booking, check_in::check_in as check_in_booking, decline_booking::decline_booking,
            get_booking::get_booking, submit_booking::{SubmitBookingCommand, submit_booking},
            validate_booking::validate_booking,
        },
    },
    infrastructure::{
        http::{
            auth::CurrentUser,
            dto::{BookingDto, Json, RoomDto, SubmitBookingBody, SubmitBookingResponse},
            error::ApiError,
        },
        state::{Adapters, AppState},
    },
};

type AppStateRef<A> = State<Arc<AppState<A>>>;

/// `GET /rooms` : le catalogue des salles (public).
/// Simple lecture sans règle métier : on interroge directement le port.
pub async fn list_rooms<A: Adapters>(State(state): AppStateRef<A>) -> Result<Json<Vec<RoomDto>>, ApiError> {
    let rooms = state.rooms.find_all().await.map_err(ApplicationError::from)?;
    Ok(Json(rooms.iter().map(RoomDto::from).collect()))
}

/// `POST /bookings` → 201 + en-tête `Location`.
pub async fn submit<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(applicant_id): CurrentUser,
    Json(body): Json<SubmitBookingBody>,
) -> Result<impl IntoResponse, ApiError> {
    let command = SubmitBookingCommand {
        id: Uuid::new_v4().to_string(),
        applicant_id,
        room_ids: body.room_ids,
        participants: body.participants,
        event_date: body.event_date,
        event_start: body.event_start,
        event_end: body.event_end,
    };

    let outcome = {
        let _writes = state.lock_writes().await;
        submit_booking(command, &state.users, &state.rooms, &state.bookings, &state.clock, &state.notifier).await?
    };

    let response = SubmitBookingResponse::from(&outcome);
    let location = format!("/bookings/{}", response.booking.id);
    Ok((StatusCode::CREATED, [(header::LOCATION, location)], Json(response)))
}

/// `GET /bookings/{id}` : visible par son demandeur et par les responsables.
pub async fn show<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(viewer_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let booking = get_booking(&id, &viewer_id, &state.users, &state.bookings).await?;
    Ok(Json(BookingDto::from(&booking)))
}

/// `POST /bookings/{id}/validate` (responsable).
pub async fn validate<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(responsible_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let _writes = state.lock_writes().await;
    let booking =
        validate_booking(&id, &responsible_id, &state.users, &state.bookings, &state.clock, &state.notifier).await?;
    Ok(Json(BookingDto::from(&booking)))
}

/// `POST /bookings/{id}/decline` (responsable).
pub async fn decline<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(responsible_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let _writes = state.lock_writes().await;
    let booking =
        decline_booking(&id, &responsible_id, &state.users, &state.bookings, &state.clock, &state.notifier).await?;
    Ok(Json(BookingDto::from(&booking)))
}

/// `POST /bookings/{id}/cancel` (demandeur de la réservation).
pub async fn cancel<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(applicant_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let _writes = state.lock_writes().await;
    let booking = cancel_booking(&id, &applicant_id, &state.bookings, &state.clock, &state.notifier).await?;
    Ok(Json(BookingDto::from(&booking)))
}

/// `POST /bookings/{id}/check-in` (demandeur de la réservation).
pub async fn check_in<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(applicant_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let _writes = state.lock_writes().await;
    let booking = check_in_booking(&id, &applicant_id, &state.bookings, &state.clock).await?;
    Ok(Json(BookingDto::from(&booking)))
}
