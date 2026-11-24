//! Traduction des erreurs de l'application en réponses HTTP.
//!
//! Corps de chaque erreur :
//! `{ "error": "<code stable pour les clients>", "message": "<texte lisible>", "alternatives": [...]? }`

use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{
    application::errors::ApplicationError,
    domain::errors::{BookingError, ResponsibleError, SystemError},
    infrastructure::http::dto::TimeSlotDto,
};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    alternatives: Option<Vec<TimeSlotDto>>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    alternatives: Option<&'a [TimeSlotDto]>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), alternatives: None }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.code,
            message: &self.message,
            alternatives: self.alternatives.as_deref(),
        };
        (self.status, Json(body)).into_response()
    }
}

/// Codes HTTP :
/// - 401 : appelant inconnu ; 403 : appelant connu mais pas autorisé ;
/// - 404 : réservation inexistante ;
/// - 409 : l'état actuel l'interdit (créneau pris, trop tard pour annuler, déjà traitée…) ;
/// - 422 : demande invalide (R1, R2, R4, salle inconnue…) ;
/// - 500 : panne du stockage (détail journalisé, jamais renvoyé au client).
impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        use ApplicationError as E;
        let message = error.to_string();
        let (status, code) = match &error {
            E::BookingNotFound(_) => (StatusCode::NOT_FOUND, "booking_not_found"),
            E::RoomNotFound(_) => (StatusCode::UNPROCESSABLE_ENTITY, "room_not_found"),
            // Dans tous les use cases, l'utilisateur chargé est l'appelant (en-tête X-User-Id).
            E::UserNotFound(_) => (StatusCode::UNAUTHORIZED, "unknown_user"),
            E::NotBookingOwner { .. } => (StatusCode::FORBIDDEN, "not_booking_owner"),

            E::Responsible(ResponsibleError::User(_)) => (StatusCode::FORBIDDEN, "missing_role"),
            E::Responsible(ResponsibleError::BookingNotRequested(_)) => {
                (StatusCode::CONFLICT, "booking_not_requested")
            }
            E::Responsible(ResponsibleError::BookingAlreadyAccepted(_)) => {
                (StatusCode::CONFLICT, "booking_already_accepted")
            }
            E::Responsible(ResponsibleError::Booking(e)) | E::Booking(e) => {
                (StatusCode::CONFLICT, booking_error_code(e))
            }

            E::System(SystemError::InvalidRequest(e)) => (StatusCode::UNPROCESSABLE_ENTITY, booking_error_code(e)),
            E::System(SystemError::SlotUnavailable { alternatives }) => {
                return Self {
                    status: StatusCode::CONFLICT,
                    code: "slot_unavailable",
                    message,
                    alternatives: Some(alternatives.iter().map(TimeSlotDto::from).collect()),
                };
            }

            E::Repository(e) => {
                eprintln!("erreur du stockage : {e}");
                return Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "erreur interne, réessayez plus tard");
            }
        };
        Self::new(status, code, message)
    }
}

/// Un code par règle. Pas de `_ =>` : une nouvelle variante oblige à choisir son code.
fn booking_error_code(error: &BookingError) -> &'static str {
    use BookingError as B;
    match error {
        B::EventEndSmallerEventStart => "end_before_start",
        B::Expired => "booking_expired",
        B::NowBeforeRequestDate => "now_before_request_date",
        B::ValideHoursAndDuration => "invalid_hours_or_duration",
        B::ParticipantsSmallerThanRoomCapacity => "capacity_exceeded",
        B::OverlapReservation => "slot_overlap",
        B::RequestTooLateOrTooEarly => "booking_window_violated",
        B::AnnulationTooLate => "cancellation_too_late",
        B::NoRoomSelected => "no_room_selected",
        B::DuplicateRoom(_) => "duplicate_room",
        B::CheckInNotApproved(_) => "check_in_not_approved",
        B::CheckInTooEarly => "check_in_too_early",
        B::CheckInTooLate => "check_in_too_late",
        B::AlreadyCheckedIn => "already_checked_in",
        B::CannotCancel(_) => "cannot_cancel",
    }
}

/// Corps JSON illisible ou incomplet : même format d'erreur que le reste de l'API
/// (au lieu du texte brut renvoyé par défaut par axum).
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::new(rejection.status(), "invalid_body", rejection.body_text())
    }
}
