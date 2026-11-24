#![allow(dead_code)]

use chrono::{NaiveDate, NaiveTime};

use crate::{
    application::ports::{BookingRepository, Clock, Notifier},
    domain::{
        applicant::Applicant,
        errors::SystemError,
        room::Room,
        system::{BookingRequest, SubmissionOutcome, System},
    },
};

/// Ce que le demandeur saisit (« Choisir la salle et le créneau »).
/// L'heure de la demande n'en fait pas partie : elle vient de l'horloge.
#[derive(Debug, Clone)]
pub struct SubmitBookingCommand {
    pub id: String,
    pub applicant: Applicant,
    pub rooms: Vec<Room>,
    pub participants: u8,
    pub event_date: NaiveDate,
    pub event_start: NaiveTime,
    pub event_end: NaiveTime,
}

/// Use case « Soumettre la demande » :
/// 1. horodate la demande ;
/// 2. charge les réservations du jour ;
/// 3. laisse le domaine appliquer R1–R5 ;
/// 4. enregistre la réservation acceptée ;
/// 5. notifie le résultat.
pub fn submit_booking(
    command: SubmitBookingCommand,
    repo: &mut impl BookingRepository,
    clock: &impl Clock,
    notifier: &impl Notifier,
) -> Result<SubmissionOutcome, SystemError> {
    let booking_id = command.id.clone();

    let request = BookingRequest {
        id: command.id,
        applicant: command.applicant,
        rooms: command.rooms,
        participants: command.participants,
        event_date: command.event_date,
        event_start: command.event_start,
        event_end: command.event_end,
        requested_at: clock.now(),
    };

    let existing = repo.find_by_date(request.event_date);

    match System::submit_request(request, &existing) {
        Ok(outcome) => {
            match &outcome {
                SubmissionOutcome::Confirmed(booking) => {
                    repo.save(booking.clone());
                    notifier.notify_confirmed(booking);
                }
                SubmissionOutcome::PendingApproval(booking) => {
                    repo.save(booking.clone());
                    notifier.notify_pending_approval(booking);
                }
            }
            Ok(outcome)
        }
        Err(error) => {
            match &error {
                SystemError::InvalidRequest(reason) => {
                    notifier.notify_rejected(&booking_id, &reason.to_string());
                }
                SystemError::SlotUnavailable { alternatives } => {
                    notifier.notify_slot_unavailable(&booking_id, alternatives);
                }
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;

    use crate::{
        application::{
            fakes::{FakeNotifier, FixedClock, InMemoryBookingRepository, Notification},
            ports::BookingRepository,
            use_cases::submit_booking::{SubmitBookingCommand, submit_booking},
        },
        domain::{
            applicant::Applicant,
            booking::{
                BookingStatus,
                tests::{amphi, booking, event_date, requested_at, salle_101, t},
            },
            errors::{BookingError, SystemError},
            room::Room,
            system::{SubmissionOutcome, TimeSlot},
        },
    };

    fn command(rooms: Vec<Room>, participants: u8, start: NaiveTime, end: NaiveTime) -> SubmitBookingCommand {
        SubmitBookingCommand {
            id: "B-NEW".into(),
            applicant: Applicant::new("A-001".into(), "User1".into()).unwrap(),
            rooms,
            participants,
            event_date: event_date(),
            event_start: start,
            event_end: end,
        }
    }

    #[test]
    fn standard_room_booking_is_confirmed_saved_and_notified() {
        let mut repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(vec![salle_101()], 20, t(10, 0), t(11, 0)), &mut repo, &clock, &notifier);

        assert!(matches!(result, Ok(SubmissionOutcome::Confirmed(_))));
        let saved = repo.find_by_id("B-NEW").expect("la réservation doit être enregistrée");
        assert_eq!(*saved.get_status(), BookingStatus::Approved);
        assert_eq!(notifier.sent(), vec![Notification::Confirmed("B-NEW".into())]);
    }

    #[test]
    fn amphi_booking_is_saved_as_pending_and_responsible_notified() {
        let mut repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(vec![amphi()], 50, t(10, 0), t(11, 0)), &mut repo, &clock, &notifier);

        assert!(matches!(result, Ok(SubmissionOutcome::PendingApproval(_))));
        assert_eq!(*repo.find_by_id("B-NEW").unwrap().get_status(), BookingStatus::Requested);
        assert_eq!(notifier.sent(), vec![Notification::PendingApproval("B-NEW".into())]);
    }

    #[test]
    fn request_time_comes_from_the_clock() {
        // Horloge réglée la veille à 15h → l'événement de 10h est à 19h : R4 refuse.
        let mut repo = InMemoryBookingRepository::default();
        let clock = FixedClock(event_date().pred_opt().unwrap().and_time(t(15, 0)));
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(vec![salle_101()], 20, t(10, 0), t(11, 0)), &mut repo, &clock, &notifier);

        assert!(matches!(result, Err(SystemError::InvalidRequest(BookingError::RequestTooLateOrTooEarly))));
    }

    #[test]
    fn invalid_booking_is_not_saved_and_rejection_is_notified() {
        let mut repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        // 40 personnes en salle 101 (30 places) → R2
        let result = submit_booking(command(vec![salle_101()], 40, t(10, 0), t(11, 0)), &mut repo, &clock, &notifier);

        assert!(matches!(
            result,
            Err(SystemError::InvalidRequest(BookingError::ParticipantsSmallerThanRoomCapacity))
        ));
        assert!(repo.find_by_id("B-NEW").is_none());
        assert!(matches!(notifier.sent().as_slice(), [Notification::Rejected(id, _)] if id == "B-NEW"));
    }

    #[test]
    fn conflicting_booking_is_not_saved_and_alternatives_are_notified() {
        let mut repo = InMemoryBookingRepository::default();
        repo.save(booking("B-001", salle_101(), BookingStatus::Approved, t(8, 0), t(10, 0)));
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(vec![salle_101()], 20, t(9, 30), t(10, 30)), &mut repo, &clock, &notifier);

        assert!(matches!(result, Err(SystemError::SlotUnavailable { .. })));
        assert!(repo.find_by_id("B-NEW").is_none());
        match notifier.sent().as_slice() {
            [Notification::SlotUnavailable(id, alternatives)] => {
                assert_eq!(id, "B-NEW");
                assert!(alternatives.contains(&TimeSlot { start: t(10, 0), end: t(11, 0) }));
            }
            other => panic!("notification inattendue : {other:?}"),
        }
    }

    #[test]
    fn second_booking_on_same_slot_conflicts_with_the_first_one() {
        let mut repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let first = submit_booking(command(vec![salle_101()], 20, t(10, 0), t(11, 0)), &mut repo, &clock, &notifier);
        let mut second_cmd = command(vec![salle_101()], 20, t(10, 0), t(11, 0));
        second_cmd.id = "B-SECOND".into();
        let second = submit_booking(second_cmd, &mut repo, &clock, &notifier);

        assert!(first.is_ok());
        assert!(matches!(second, Err(SystemError::SlotUnavailable { .. })));
        assert!(repo.find_by_id("B-SECOND").is_none());
    }
}
