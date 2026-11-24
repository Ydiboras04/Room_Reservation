#![allow(dead_code)]

use chrono::{NaiveDate, NaiveTime};

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, RoomRepository},
    },
    domain::{
        booking::{Booking, BookingStatus},
        room::Room,
    },
};

/// Planning d'une salle sur une journée.
#[derive(Debug, Clone)]
pub struct RoomAvailability {
    pub room: Room,
    pub date: NaiveDate,
    /// Créneaux déjà pris, triés par heure de début.
    pub busy: Vec<BusySlot>,
}

/// Créneau occupé. Volontairement sans l'id de la réservation ni le demandeur :
/// n'importe qui peut consulter le planning d'une salle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusySlot {
    pub start: NaiveTime,
    pub end: NaiveTime,
    /// `Requested` (en attente de validation) ou `Approved`.
    pub status: BookingStatus,
}

/// Planning d'une salle pour une date : permet à l'interface d'afficher les créneaux libres
/// au lieu de laisser le demandeur deviner et se heurter à un conflit (R3).
///
/// « Occupé » suit exactement la règle du contrôle de conflit : `Booking::blocks_slot`
/// (demande en attente non expirée, ou réservation approuvée).
pub async fn room_availability(
    room_id: &str,
    date: NaiveDate,
    rooms: &impl RoomRepository,
    repo: &impl BookingRepository,
) -> Result<RoomAvailability, ApplicationError> {
    let room = rooms
        .find_by_id(room_id)
        .await?
        .ok_or_else(|| ApplicationError::RoomNotFound(room_id.to_string()))?;

    let mut busy: Vec<BusySlot> = repo
        .find_by_date(date)
        .await?
        .iter()
        .filter(|b| b.blocks_slot() && uses_room(b, room_id))
        .map(|b| BusySlot { start: b.get_event_start(), end: b.get_event_end(), status: *b.get_status() })
        .collect();
    busy.sort_by_key(|slot| slot.start);

    Ok(RoomAvailability { room, date, busy })
}

fn uses_room(booking: &Booking, room_id: &str) -> bool {
    booking.get_rooms().iter().any(|r| r.get_id() == room_id)
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{InMemoryBookingRepository, InMemoryRoomRepository},
            use_cases::room_availability::{BusySlot, room_availability},
        },
        domain::booking::{
            BookingStatus,
            tests::{amphi, booking, event_date, salle_101, t},
        },
    };

    fn rooms() -> InMemoryRoomRepository {
        InMemoryRoomRepository::with(vec![salle_101(), amphi()])
    }

    #[tokio::test]
    async fn busy_slots_are_the_blocking_bookings_of_that_room_sorted_by_start() {
        let mut approved = booking("B-1", salle_101(), BookingStatus::Requested, t(14, 0), t(15, 0));
        approved.approve();
        let repo = InMemoryBookingRepository::with(vec![
            approved,
            booking("B-2", salle_101(), BookingStatus::Requested, t(9, 0), t(10, 0)),
            booking("B-3", salle_101(), BookingStatus::Cancelled, t(11, 0), t(12, 0)), // ne bloque pas
            booking("B-4", amphi(), BookingStatus::Requested, t(9, 0), t(10, 0)),      // autre salle
        ]);

        let availability = room_availability("R-101", event_date(), &rooms(), &repo).await.unwrap();

        assert_eq!(availability.room.get_id(), "R-101");
        assert_eq!(
            availability.busy,
            vec![
                BusySlot { start: t(9, 0), end: t(10, 0), status: BookingStatus::Requested },
                BusySlot { start: t(14, 0), end: t(15, 0), status: BookingStatus::Approved },
            ]
        );
    }

    #[tokio::test]
    async fn other_days_are_ignored() {
        let repo = InMemoryBookingRepository::with(vec![booking(
            "B-1",
            salle_101(),
            BookingStatus::Requested,
            t(9, 0),
            t(10, 0),
        )]);

        let next_day = event_date() + TimeDelta::days(1);
        let availability = room_availability("R-101", next_day, &rooms(), &repo).await.unwrap();

        assert!(availability.busy.is_empty());
    }

    #[tokio::test]
    async fn unknown_room_returns_room_not_found() {
        let result = room_availability("R-999", event_date(), &rooms(), &InMemoryBookingRepository::default()).await;

        assert!(matches!(result, Err(ApplicationError::RoomNotFound(id)) if id == "R-999"));
    }
}
