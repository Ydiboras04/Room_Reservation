#![allow(dead_code)]

//! Couloir « Système » du BPMN : enchaîne les vérifications d'une demande
//! et les contrôles périodiques (expiration, rappel, check-in, fin d'événement).

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

use crate::domain::{
    booking::{Booking, BookingStatus, CLOSING_HOUR, OPENING_HOUR, SLOT_STEP_MINUTES},
    errors::{BookingError, SystemError},
    room::Room,
    user::User,
};

/// Un créneau horaire proposé au demandeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSlot {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

/// Données saisies par le demandeur (« Choisir la salle et le créneau »).
#[derive(Debug, Clone)]
pub struct BookingRequest {
    pub id: String,
    pub applicant: User,
    pub rooms: Vec<Room>,
    pub participants: u8,
    pub event_date: NaiveDate,
    pub event_start: NaiveTime,
    pub event_end: NaiveTime,
    pub requested_at: NaiveDateTime,
}

/// Issue d'une demande acceptée par le système.
#[derive(Debug)]
pub enum SubmissionOutcome {
    /// Salles standard : « Confirmer et bloquer le créneau ».
    Confirmed(Booking),
    /// Au moins un amphithéâtre : « Valider la demande » par le responsable.
    PendingApproval(Booking),
}

pub struct System;

impl System {
    /// « Soumettre la demande » → R1, R2, R4 → R3 → type de salle.
    /// `existing` : réservations déjà enregistrées (fournies par le dépôt).
    pub fn submit_request(request: BookingRequest, existing: &[Booking]) -> Result<SubmissionOutcome, SystemError> {
        // « Vérifier les règles R1, R2, R4 » → « Demande valide ? »
        let mut booking = Booking::new(
            request.id,
            request.applicant,
            request.rooms,
            BookingStatus::Requested,
            request.participants,
            request.event_date,
            request.event_start,
            request.event_end,
            request.requested_at,
        )?;

        // « Vérifier les conflits (R3) » → « Créneau libre ? »
        if let Err(BookingError::OverlapReservation) = booking.ensure_no_conflict(existing) {
            return Err(SystemError::SlotUnavailable {
                alternatives: Self::propose_alternative_slots(&booking, existing),
            });
        }

        // « Type de salle ? » (R5)
        booking.auto_validate();
        if *booking.get_status() == BookingStatus::Approved {
            Ok(SubmissionOutcome::Confirmed(booking))
        } else {
            Ok(SubmissionOutcome::PendingApproval(booking))
        }
    }

    /// « Proposer d'autres créneaux » : même jour, mêmes salles, même durée,
    /// créneaux respectant R1 (7h–19h, pas de 30 min), R4 (délai) et R3 (sans conflit).
    pub fn propose_alternative_slots(booking: &Booking, existing: &[Booking]) -> Vec<TimeSlot> {
        let duration = booking.get_event_end() - booking.get_event_start();
        let step = TimeDelta::minutes(SLOT_STEP_MINUTES);
        let closing = NaiveTime::from_hms_opt(CLOSING_HOUR, 0, 0).unwrap();

        let mut slots = Vec::new();
        let mut start = NaiveTime::from_hms_opt(OPENING_HOUR, 0, 0).unwrap();

        while start + duration <= closing && start + duration > start {
            let end = start + duration;
            let is_requested_slot = start == booking.get_event_start();

            if !is_requested_slot {
                if let Ok(candidate) = booking.with_slot(start, end) {
                    if candidate.ensure_no_conflict(existing).is_ok() {
                        slots.push(TimeSlot { start, end });
                    }
                }
            }
            start += step;
        }
        slots
    }

    /// Contrôles périodiques, à lancer régulièrement par la couche application :
    /// - expiration (24h sans réponse) ;
    /// - libération des salles (fin d'événement, 15 min sans check-in) ;
    /// - renvoie les ids des réservations dont le rappel (1h avant) est dû.
    ///
    /// L'infrastructure envoie les rappels puis appelle `mark_reminder_sent`.
    pub fn run_scheduled_checks(bookings: &mut [Booking], now: NaiveDateTime) -> Vec<String> {
        let mut reminders = Vec::new();
        for booking in bookings.iter_mut() {
            // Une date « now » antérieure à la demande est ignorée ici (erreur d'horloge).
            let _ = booking.update_expiry(now);
            booking.update_room_availability(now);

            if booking.reminder_due(now) {
                reminders.push(booking.get_id().to_string());
            }
        }
        reminders
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveTime, TimeDelta};

    use crate::domain::{
            booking::{
            Booking, BookingStatus,
            tests::{amphi, booking, event_date, requested_at, salle_101, t},
        },
        errors::{BookingError, SystemError},
        room::Room,
        system::{BookingRequest, SubmissionOutcome, System, TimeSlot},
        user::tests::applicant,
    };

    fn request(rooms: Vec<Room>, participants: u8, start: NaiveTime, end: NaiveTime) -> BookingRequest {
        BookingRequest {
            id: "B-NEW".into(),
            applicant: applicant(),
            rooms,
            participants,
            event_date: event_date(),
            event_start: start,
            event_end: end,
            requested_at: requested_at(),
        }
    }

    // ---------- submit_request ----------

    #[test]
    fn standard_room_request_is_confirmed_and_blocks_rooms() {
        let outcome = System::submit_request(request(vec![salle_101()], 20, t(10, 0), t(11, 0)), &[]).unwrap();

        match outcome {
            SubmissionOutcome::Confirmed(b) => {
                assert_eq!(*b.get_status(), BookingStatus::Approved);
                assert!(b.get_rooms().iter().all(|r| !r.is_free()));
            }
            other => panic!("attendu Confirmed, obtenu {other:?}"),
        }
    }

    #[test]
    fn amphi_request_waits_for_approval() {
        let outcome = System::submit_request(request(vec![amphi()], 50, t(10, 0), t(11, 0)), &[]).unwrap();

        assert!(matches!(outcome, SubmissionOutcome::PendingApproval(b) if *b.get_status() == BookingStatus::Requested));
    }

    #[test]
    fn invalid_request_is_rejected() {
        // 40 personnes en salle 101 (30 places) → R2
        let result = System::submit_request(request(vec![salle_101()], 40, t(10, 0), t(11, 0)), &[]);

        assert!(matches!(
            result,
            Err(SystemError::InvalidRequest(BookingError::ParticipantsSmallerThanRoomCapacity))
        ));
    }

    #[test]
    fn conflicting_request_returns_alternative_slots() {
        let existing = vec![booking("B-001", salle_101(), BookingStatus::Approved, t(8, 0), t(10, 0))];

        let result = System::submit_request(request(vec![salle_101()], 20, t(9, 30), t(10, 30)), &existing);

        let Err(SystemError::SlotUnavailable { alternatives }) = result else {
            panic!("attendu SlotUnavailable, obtenu {result:?}");
        };
        assert_eq!(alternatives.first(), Some(&TimeSlot { start: t(7, 0), end: t(8, 0) }));
        assert!(alternatives.contains(&TimeSlot { start: t(10, 0), end: t(11, 0) }));
        assert_eq!(alternatives.last(), Some(&TimeSlot { start: t(18, 0), end: t(19, 0) }));
        // aucun créneau proposé ne chevauche 08:00–10:00
        assert!(alternatives.iter().all(|s| s.end <= t(8, 0) || s.start >= t(10, 0)));
    }

    // ---------- propose_alternative_slots ----------

    #[test]
    fn alternative_slots_keep_the_requested_duration() {
        let existing = vec![booking("B-001", salle_101(), BookingStatus::Approved, t(8, 0), t(10, 0))];
        let wanted = booking("B-NEW", salle_101(), BookingStatus::Requested, t(9, 0), t(11, 0));

        let slots = System::propose_alternative_slots(&wanted, &existing);

        assert!(!slots.is_empty());
        assert!(slots.iter().all(|s| s.end - s.start == TimeDelta::hours(2)));
    }

    #[test]
    fn alternative_slots_respect_lead_time_r4() {
        // Demande faite la veille à 11h : tout créneau avant 11h le jour J est à moins de 24h.
        let requested = event_date().pred_opt().unwrap().and_time(t(11, 0));
        let wanted = Booking::new(
            "B-NEW".into(),
            applicant(),
            vec![salle_101()],
            BookingStatus::Requested,
            10,
            event_date(),
            t(14, 0),
            t(15, 0),
            requested,
        )
        .unwrap();

        let slots = System::propose_alternative_slots(&wanted, &[]);

        assert_eq!(slots.first(), Some(&TimeSlot { start: t(11, 0), end: t(12, 0) }));
    }

    // ---------- run_scheduled_checks ----------

    #[test]
    fn scheduled_checks_return_due_reminders() {
        let mut b = booking("B-001", salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0));
        b.approve();
        let mut bookings = vec![b];

        let due = System::run_scheduled_checks(&mut bookings, event_date().and_time(t(9, 15)));

        assert_eq!(due, vec!["B-001".to_string()]);
    }

    #[test]
    fn scheduled_checks_expire_old_pending_requests() {
        let mut bookings = vec![booking("B-001", amphi(), BookingStatus::Requested, t(10, 0), t(12, 0))];

        System::run_scheduled_checks(&mut bookings, requested_at() + TimeDelta::hours(24));

        assert!(bookings[0].is_expired());
    }

    #[test]
    fn scheduled_checks_release_rooms_on_no_show() {
        let mut b = booking("B-001", salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0));
        b.approve();
        let mut bookings = vec![b];

        System::run_scheduled_checks(&mut bookings, event_date().and_time(t(10, 16)));

        assert_eq!(*bookings[0].get_status(), BookingStatus::NoShow);
        assert!(bookings[0].get_rooms().iter().all(|r| r.is_free()));
    }
}
