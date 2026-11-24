//! Formats JSON de l'API. Le domaine ne dépend pas de serde : la conversion se fait ici,
//! ce qui permet de faire évoluer le JSON et les entités indépendamment.
//!
//! Chaque type dérive aussi `ToSchema` / `IntoParams` : c'est ce qui alimente la
//! documentation OpenAPI (voir `openapi.rs`), qui reste ainsi alignée sur le vrai JSON.

use axum::extract::{FromRequest, FromRequestParts};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    application::use_cases::room_availability::{BusySlot, RoomAvailability},
    domain::{
        booking::{Booking, BookingStatus, CLOSING_HOUR, OPENING_HOUR},
        room::{Room, RoomType},
        system::{SubmissionOutcome, TimeSlot},
        user::{User, UserRole},
    },
    infrastructure::http::error::ApiError,
};

// ---------- extracteurs ----------

/// `axum::Json`, mais dont les rejets utilisent le format d'erreur de l'API.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(pub T);

impl<T: Serialize> axum::response::IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

/// `axum::extract::Query`, mais dont les rejets utilisent le format d'erreur de l'API.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

// ---------- énumérations ----------

/// Statut d'une réservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BookingStatusDto {
    /// En attente de validation par un responsable (amphithéâtre).
    Requested,
    Approved,
    Declined,
    Cancelled,
    /// Pas de check-in dans les 15 min après le début : salle libérée.
    NoShow,
}

impl From<BookingStatus> for BookingStatusDto {
    fn from(status: BookingStatus) -> Self {
        match status {
            BookingStatus::Requested => Self::Requested,
            BookingStatus::Approved => Self::Approved,
            BookingStatus::Declined => Self::Declined,
            BookingStatus::Cancelled => Self::Cancelled,
            BookingStatus::NoShow => Self::NoShow,
        }
    }
}

impl From<BookingStatusDto> for BookingStatus {
    fn from(status: BookingStatusDto) -> Self {
        match status {
            BookingStatusDto::Requested => Self::Requested,
            BookingStatusDto::Approved => Self::Approved,
            BookingStatusDto::Declined => Self::Declined,
            BookingStatusDto::Cancelled => Self::Cancelled,
            BookingStatusDto::NoShow => Self::NoShow,
        }
    }
}

/// Type de salle : un amphithéâtre exige la validation d'un responsable (R5).
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomKindDto {
    Standard,
    Amphi,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserRoleDto {
    Applicant,
    Responsible,
}

/// Issue d'une soumission acceptée.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeDto {
    /// Salle standard : réservation approuvée immédiatement.
    Confirmed,
    /// Amphithéâtre : en attente de validation par un responsable.
    PendingApproval,
}

// ---------- requêtes ----------

/// Corps de `POST /bookings`. Le demandeur vient de l'en-tête `X-User-Id`,
/// l'id de la réservation est généré par le serveur.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitBookingBody {
    #[schema(example = json!(["R-101"]))]
    pub room_ids: Vec<String>,
    #[schema(example = 20)]
    pub participants: u8,
    #[schema(example = "2026-12-01")]
    pub event_date: NaiveDate,
    /// `"10:00"` ou `"10:00:00"`.
    #[schema(value_type = String, example = "10:00")]
    pub event_start: NaiveTime,
    #[schema(value_type = String, example = "11:00")]
    pub event_end: NaiveTime,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct ListBookingsQuery {
    /// Ne garder que ce statut (ex. `requested` pour les demandes à valider).
    pub status: Option<BookingStatusDto>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct AvailabilityQuery {
    /// Jour consulté.
    #[param(example = "2026-12-01")]
    pub date: NaiveDate,
}

// ---------- réponses ----------

#[derive(Debug, Serialize, ToSchema)]
pub struct BookingDto {
    pub id: String,
    pub applicant_id: String,
    pub room_ids: Vec<String>,
    pub participants: u8,
    pub event_date: NaiveDate,
    #[schema(value_type = String, example = "10:00:00")]
    pub event_start: NaiveTime,
    #[schema(value_type = String, example = "11:00:00")]
    pub event_end: NaiveTime,
    pub status: BookingStatusDto,
    pub requested_at: NaiveDateTime,
    /// Demande restée 24 h sans réponse (le statut reste `requested`).
    pub expired: bool,
    pub checked_in: bool,
}

impl From<&Booking> for BookingDto {
    fn from(booking: &Booking) -> Self {
        Self {
            id: booking.get_id().to_string(),
            applicant_id: booking.get_applicant().get_id().to_string(),
            room_ids: booking.get_rooms().iter().map(|r| r.get_id().to_string()).collect(),
            participants: booking.get_participants(),
            event_date: booking.get_event_date(),
            event_start: booking.get_event_start(),
            event_end: booking.get_event_end(),
            status: (*booking.get_status()).into(),
            requested_at: booking.get_requested_at(),
            expired: booking.is_expired(),
            checked_in: booking.is_checked_in(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitBookingResponse {
    pub outcome: OutcomeDto,
    pub booking: BookingDto,
}

impl From<&SubmissionOutcome> for SubmitBookingResponse {
    fn from(outcome: &SubmissionOutcome) -> Self {
        let (outcome, booking) = match outcome {
            SubmissionOutcome::Confirmed(b) => (OutcomeDto::Confirmed, b),
            SubmissionOutcome::PendingApproval(b) => (OutcomeDto::PendingApproval, b),
        };
        Self { outcome, booking: booking.into() }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoomDto {
    #[schema(example = "R-101")]
    pub id: String,
    #[schema(example = "Salle 101")]
    pub name: String,
    #[schema(example = 30)]
    pub capacity: u8,
    pub kind: RoomKindDto,
}

impl From<&Room> for RoomDto {
    fn from(room: &Room) -> Self {
        Self {
            id: room.get_id().to_string(),
            name: room.get_name().to_string(),
            capacity: room.get_capacity(),
            kind: match room.get_kind() {
                RoomType::Standard => RoomKindDto::Standard,
                RoomType::Amphi => RoomKindDto::Amphi,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TimeSlotDto {
    #[schema(value_type = String, example = "11:00:00")]
    pub start: NaiveTime,
    #[schema(value_type = String, example = "12:00:00")]
    pub end: NaiveTime,
}

impl From<&TimeSlot> for TimeSlotDto {
    fn from(slot: &TimeSlot) -> Self {
        Self { start: slot.start, end: slot.end }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserDto {
    #[schema(example = "A-001")]
    pub id: String,
    #[schema(example = "Alice")]
    pub name: String,
    pub role: UserRoleDto,
}

impl From<&User> for UserDto {
    fn from(user: &User) -> Self {
        Self {
            id: user.get_id().to_string(),
            name: user.get_name().to_string(),
            role: match user.get_role() {
                UserRole::Applicant => UserRoleDto::Applicant,
                UserRole::Responsible => UserRoleDto::Responsible,
            },
        }
    }
}

/// Planning d'une salle pour une journée. `opening` rappelle les horaires (R1)
/// pour que l'interface puisse dessiner la journée et en déduire les créneaux libres.
#[derive(Debug, Serialize, ToSchema)]
pub struct AvailabilityDto {
    pub room: RoomDto,
    pub date: NaiveDate,
    pub opening: TimeSlotDto,
    /// Créneaux occupés, triés par heure de début.
    pub busy: Vec<BusySlotDto>,
}

/// Créneau occupé (sans le demandeur ni l'id de la réservation).
#[derive(Debug, Serialize, ToSchema)]
pub struct BusySlotDto {
    #[schema(value_type = String, example = "10:00:00")]
    pub start: NaiveTime,
    #[schema(value_type = String, example = "12:00:00")]
    pub end: NaiveTime,
    /// `requested` (en attente) ou `approved`.
    pub status: BookingStatusDto,
}

impl From<&RoomAvailability> for AvailabilityDto {
    fn from(availability: &RoomAvailability) -> Self {
        let hour = |h| NaiveTime::from_hms_opt(h, 0, 0).expect("heure d'ouverture valide");
        Self {
            room: RoomDto::from(&availability.room),
            date: availability.date,
            opening: TimeSlotDto { start: hour(OPENING_HOUR), end: hour(CLOSING_HOUR) },
            busy: availability.busy.iter().map(BusySlotDto::from).collect(),
        }
    }
}

impl From<&BusySlot> for BusySlotDto {
    fn from(slot: &BusySlot) -> Self {
        Self { start: slot.start, end: slot.end, status: slot.status.into() }
    }
}
