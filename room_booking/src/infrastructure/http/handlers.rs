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
        ports::{RoomRepository, UserRepository},
        use_cases::{
            cancel_booking::cancel_booking, check_in::check_in as check_in_booking, decline_booking::decline_booking,
            get_booking::get_booking, list_bookings::list_bookings, list_my_bookings::list_my_bookings,
            room_availability::room_availability, submit_booking::{SubmitBookingCommand, submit_booking},
            validate_booking::validate_booking,
        },
    },
    infrastructure::{
        http::{
            auth::CurrentUser,
            dto::{
                AvailabilityDto, AvailabilityQuery, BookingDto, Json, ListBookingsQuery, Query, RoomDto,
                SubmitBookingBody, SubmitBookingResponse, UserDto,
            },
            error::{ApiError, ErrorBody},
        },
        state::{Adapters, AppState},
    },
};

type AppStateRef<A> = State<Arc<AppState<A>>>;

/// Lister les salles
///
/// Catalogue public. Simple lecture sans règle métier : le handler interroge directement le port.
#[utoipa::path(
    get, path = "/rooms", tag = "Salles", operation_id = "list_rooms",
    responses((status = 200, body = Vec<RoomDto>), (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody))
)]
pub async fn list_rooms<A: Adapters>(State(state): AppStateRef<A>) -> Result<Json<Vec<RoomDto>>, ApiError> {
    let rooms = state.rooms.find_all().await.map_err(ApplicationError::from)?;
    Ok(Json(rooms.iter().map(RoomDto::from).collect()))
}

/// Disponibilités d'une salle
///
/// Créneaux occupés de la salle ce jour-là (demandes en attente non expirées et réservations
/// approuvées), plus les horaires d'ouverture. Public : n'expose ni demandeur ni id de réservation.
#[utoipa::path(
    get, path = "/rooms/{id}/availability", tag = "Salles", operation_id = "room_availability",
    params(("id" = String, Path, description = "Id de la salle", example = "R-101"), AvailabilityQuery),
    responses(
        (status = 200, body = AvailabilityDto),
        (status = 400, description = "`invalid_query` : date absente ou mal formée", body = ErrorBody),
        (status = 404, description = "`room_not_found`", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
pub async fn room_availability_handler<A: Adapters>(
    State(state): AppStateRef<A>,
    Path(room_id): Path<String>,
    Query(query): Query<AvailabilityQuery>,
) -> Result<Json<AvailabilityDto>, ApiError> {
    let availability = room_availability(&room_id, query.date, &state.rooms, &state.bookings)
        .await
        .map_err(|error| match error {
            // Ici la salle est la ressource demandée (dans le chemin) : 404, et non 422
            // comme lorsqu'elle est citée dans le corps d'une demande de réservation.
            ApplicationError::RoomNotFound(_) => ApiError::new(StatusCode::NOT_FOUND, "room_not_found", error.to_string()),
            other => other.into(),
        })?;
    Ok(Json(AvailabilityDto::from(&availability)))
}

/// Lister les utilisateurs
///
/// Alimente l'écran « se connecter en tant que… ». **Provisoire** : à retirer (ou à réserver
/// aux administrateurs) avec la vraie authentification.
#[utoipa::path(
    get, path = "/users", tag = "Utilisateurs", operation_id = "list_users",
    responses((status = 200, body = Vec<UserDto>), (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody))
)]
pub async fn list_users<A: Adapters>(State(state): AppStateRef<A>) -> Result<Json<Vec<UserDto>>, ApiError> {
    let users = state.users.find_all().await.map_err(ApplicationError::from)?;
    Ok(Json(users.iter().map(UserDto::from).collect()))
}

/// Mes réservations
///
/// Toutes les réservations de l'appelant, quel que soit leur statut, par ordre chronologique.
#[utoipa::path(
    get, path = "/bookings/mine", tag = "Réservations", operation_id = "list_my_bookings", security(("user_id" = [])),
    responses((status = 200, body = Vec<BookingDto>), (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody), (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody))
)]
pub async fn list_mine<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(user_id): CurrentUser,
) -> Result<Json<Vec<BookingDto>>, ApiError> {
    let bookings = list_my_bookings(&user_id, &state.users, &state.bookings).await?;
    Ok(Json(bookings.iter().map(BookingDto::from).collect()))
}

/// Lister les réservations (responsable)
///
/// Toutes les réservations, filtrables par statut. `?status=requested` donne les demandes à valider ;
/// les demandes expirées y figurent avec `expired: true`.
#[utoipa::path(
    get, path = "/bookings", tag = "Réservations", operation_id = "list_bookings", security(("user_id" = [])),
    params(ListBookingsQuery),
    responses(
        (status = 200, body = Vec<BookingDto>),
        (status = 400, description = "`invalid_query` : statut inconnu", body = ErrorBody),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 403, description = "`missing_role` : réservé aux responsables", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
pub async fn list_all<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(viewer_id): CurrentUser,
    Query(query): Query<ListBookingsQuery>,
) -> Result<Json<Vec<BookingDto>>, ApiError> {
    let bookings = list_bookings(&viewer_id, query.status.map(Into::into), &state.users, &state.bookings).await?;
    Ok(Json(bookings.iter().map(BookingDto::from).collect()))
}

/// Soumettre une demande de réservation
///
/// Le demandeur est l'appelant (X-User-Id) ; capacité et type des salles viennent du catalogue.
/// Salle standard → approuvée immédiatement ; amphithéâtre → en attente d'un responsable.
#[utoipa::path(
    post, path = "/bookings", tag = "Réservations", operation_id = "submit_booking", security(("user_id" = [])),
    request_body = SubmitBookingBody,
    responses(
        (status = 201, body = SubmitBookingResponse,
            headers(("Location" = String, description = "Chemin de la réservation créée : /bookings/{id}"))),
        (status = 400, description = "`invalid_body` : JSON illisible", body = ErrorBody),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 409, description = "`slot_unavailable` (R3) : créneau pris, `alternatives` propose des créneaux libres", body = ErrorBody),
        (status = 422, description = "Demande invalide : `invalid_body` (champ manquant ou inconnu) · `room_not_found` · \
            `no_room_selected` · `duplicate_room` · `capacity_exceeded` (R2) · `invalid_hours_or_duration` (R1 : 7h–19h, 30 min à 4 h) · \
            `end_before_start` · `booking_window_violated` (R4 : 24 h à 30 jours à l'avance)", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
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

/// Consulter une réservation
///
/// Visible par son demandeur et par les responsables.
#[utoipa::path(
    get, path = "/bookings/{id}", tag = "Réservations", operation_id = "get_booking", security(("user_id" = [])),
    params(("id" = String, Path, description = "Id de la réservation")),
    responses(
        (status = 200, body = BookingDto),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 403, description = "`not_booking_owner`", body = ErrorBody),
        (status = 404, description = "`booking_not_found`", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
pub async fn show<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(viewer_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let booking = get_booking(&id, &viewer_id, &state.users, &state.bookings).await?;
    Ok(Json(BookingDto::from(&booking)))
}

/// Valider une demande (responsable)
///
/// Uniquement une demande en attente, non expirée (24 h sans réponse).
#[utoipa::path(
    post, path = "/bookings/{id}/validate", tag = "Réservations", operation_id = "validate_booking", security(("user_id" = [])),
    params(("id" = String, Path, description = "Id de la réservation")),
    responses(
        (status = 200, description = "Réservation approuvée", body = BookingDto),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 403, description = "`missing_role` : réservé aux responsables", body = ErrorBody),
        (status = 404, description = "`booking_not_found`", body = ErrorBody),
        (status = 409, description = "`booking_not_requested` · `booking_already_accepted` · `booking_expired`", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
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

/// Refuser une demande (responsable)
///
/// Uniquement une demande en attente, non expirée. Libère le créneau.
#[utoipa::path(
    post, path = "/bookings/{id}/decline", tag = "Réservations", operation_id = "decline_booking", security(("user_id" = [])),
    params(("id" = String, Path, description = "Id de la réservation")),
    responses(
        (status = 200, description = "Réservation refusée", body = BookingDto),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 403, description = "`missing_role` : réservé aux responsables", body = ErrorBody),
        (status = 404, description = "`booking_not_found`", body = ErrorBody),
        (status = 409, description = "`booking_not_requested` · `booking_already_accepted` · `booking_expired`", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
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

/// Annuler sa réservation
///
/// Par son demandeur, au plus tard 2 h avant le début (R6). Libère le créneau.
#[utoipa::path(
    post, path = "/bookings/{id}/cancel", tag = "Réservations", operation_id = "cancel_booking", security(("user_id" = [])),
    params(("id" = String, Path, description = "Id de la réservation")),
    responses(
        (status = 200, description = "Réservation annulée", body = BookingDto),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 403, description = "`not_booking_owner`", body = ErrorBody),
        (status = 404, description = "`booking_not_found`", body = ErrorBody),
        (status = 409, description = "`cancellation_too_late` (R6) · `cannot_cancel` : déjà refusée, annulée ou non présentée", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
pub async fn cancel<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(applicant_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let _writes = state.lock_writes().await;
    let booking = cancel_booking(&id, &applicant_id, &state.bookings, &state.clock, &state.notifier).await?;
    Ok(Json(BookingDto::from(&booking)))
}

/// Faire le check-in
///
/// Par le demandeur, entre le début de l'événement et 15 min après, sur une réservation approuvée.
/// Trop tard : la réservation passe en `no_show` et les salles sont libérées.
#[utoipa::path(
    post, path = "/bookings/{id}/check-in", tag = "Réservations", operation_id = "check_in", security(("user_id" = [])),
    params(("id" = String, Path, description = "Id de la réservation")),
    responses(
        (status = 200, description = "Check-in enregistré", body = BookingDto),
        (status = 401, description = "`missing_user` : en-tête X-User-Id absent · `unknown_user` : utilisateur inconnu", body = ErrorBody),
        (status = 403, description = "`not_booking_owner`", body = ErrorBody),
        (status = 404, description = "`booking_not_found`", body = ErrorBody),
        (status = 409, description = "`check_in_not_approved` · `check_in_too_early` · `check_in_too_late` (réservation passée en no_show) · `already_checked_in`", body = ErrorBody),
        (status = 500, description = "`internal_error` : panne du stockage", body = ErrorBody),
    )
)]
pub async fn check_in<A: Adapters>(
    State(state): AppStateRef<A>,
    CurrentUser(applicant_id): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<BookingDto>, ApiError> {
    let _writes = state.lock_writes().await;
    let booking = check_in_booking(&id, &applicant_id, &state.bookings, &state.clock).await?;
    Ok(Json(BookingDto::from(&booking)))
}
