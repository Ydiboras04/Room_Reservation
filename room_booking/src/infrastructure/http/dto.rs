//! Formats JSON de l'API. Le domaine ne dépend pas de serde : la conversion se fait ici,
//! ce qui permet de faire évoluer le JSON et les entités indépendamment.

use axum::extract::FromRequest;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        booking::{Booking, BookingStatus},
        room::{Room, RoomType},
        system::{SubmissionOutcome, TimeSlot},
    },
    infrastructure::http::error::ApiError,
};

/// `axum::Json`, mais dont les rejets utilisent le format d'erreur de l'API.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(pub T);

impl<T: Serialize> axum::response::IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

/// Corps de `POST /bookings`. Le demandeur vient de l'en-tête `X-User-Id`,
/// l'id de la réservation est généré par le serveur.
/// Heures acceptées : `"10:00"` ou `"10:00:00"`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitBookingBody {
    pub room_ids: Vec<String>,
    pub participants: u8,
    pub event_date: NaiveDate,
    pub event_start: NaiveTime,
    pub event_end: NaiveTime,
}

#[derive(Debug, Serialize)]
pub struct BookingDto {
    pub id: String,
    pub applicant_id: String,
    pub room_ids: Vec<String>,
    pub participants: u8,
    pub event_date: NaiveDate,
    pub event_start: NaiveTime,
    pub event_end: NaiveTime,
    pub status: &'static str,
    pub requested_at: NaiveDateTime,
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
            status: status_name(*booking.get_status()),
            requested_at: booking.get_requested_at(),
            expired: booking.is_expired(),
            checked_in: booking.is_checked_in(),
        }
    }
}

fn status_name(status: BookingStatus) -> &'static str {
    match status {
        BookingStatus::Requested => "requested",
        BookingStatus::Approved => "approved",
        BookingStatus::Declined => "declined",
        BookingStatus::Cancelled => "cancelled",
        BookingStatus::NoShow => "no_show",
    }
}

/// Réponse de `POST /bookings` : `outcome` vaut `"confirmed"` (salle standard)
/// ou `"pending_approval"` (amphithéâtre, en attente du responsable).
#[derive(Debug, Serialize)]
pub struct SubmitBookingResponse {
    pub outcome: &'static str,
    pub booking: BookingDto,
}

impl From<&SubmissionOutcome> for SubmitBookingResponse {
    fn from(outcome: &SubmissionOutcome) -> Self {
        let (outcome, booking) = match outcome {
            SubmissionOutcome::Confirmed(b) => ("confirmed", b),
            SubmissionOutcome::PendingApproval(b) => ("pending_approval", b),
        };
        Self { outcome, booking: booking.into() }
    }
}

#[derive(Debug, Serialize)]
pub struct RoomDto {
    pub id: String,
    pub name: String,
    pub capacity: u8,
    pub kind: &'static str,
}

impl From<&Room> for RoomDto {
    fn from(room: &Room) -> Self {
        Self {
            id: room.get_id().to_string(),
            name: room.get_name().to_string(),
            capacity: room.get_capacity(),
            kind: match room.get_kind() {
                RoomType::Standard => "standard",
                RoomType::Amphi => "amphi",
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeSlotDto {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl From<&TimeSlot> for TimeSlotDto {
    fn from(slot: &TimeSlot) -> Self {
        Self { start: slot.start, end: slot.end }
    }
}
