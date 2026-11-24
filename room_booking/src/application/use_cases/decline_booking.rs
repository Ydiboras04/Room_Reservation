#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, Clock, Notifier},
        use_cases::common::load_booking,
    },
    domain::{booking::Booking, responsible::Responsible},
};

/// Use case « Valider la demande (amphithéâtre) » → « Approuvée ? → Non » → Refusée :
/// 1. charge la réservation ;
/// 2. le responsable la refuse (impossible si expirée après 24h, déjà acceptée, ou plus en attente) ;
/// 3. enregistre le nouvel état ;
/// 4. notifie le demandeur du refus.
pub fn decline_booking(
    booking_id: &str,
    responsible: &Responsible,
    repo: &mut impl BookingRepository,
    clock: &impl Clock,
    notifier: &impl Notifier,
) -> Result<Booking, ApplicationError> {
    let mut booking = load_booking(repo, booking_id)?;

    let result = responsible.decline_request(&mut booking, clock.now());

    // Même en cas d'erreur, la réservation a pu être marquée expirée : on enregistre son état.
    repo.save(booking.clone());

    result?;
    notifier.notify_declined(&booking);
    Ok(booking)
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{FakeNotifier, FixedClock, InMemoryBookingRepository, Notification},
            ports::BookingRepository,
            use_cases::decline_booking::decline_booking,
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{amphi, booking, requested_at, t},
            },
            errors::{BookingError, ResponsibleError},
            responsible::Responsible,
        },
    };

    fn responsible() -> Responsible {
        Responsible::new("RESP-001".into(), "Chef".into()).unwrap()
    }

    fn repo_with(status: BookingStatus) -> InMemoryBookingRepository {
        let mut repo = InMemoryBookingRepository::default();
        repo.save(booking("B-001", amphi(), status, t(10, 0), t(12, 0)));
        repo
    }

    #[test]
    fn pending_booking_is_declined_saved_and_notified() {
        let mut repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = decline_booking("B-001", &responsible(), &mut repo, &clock, &notifier);

        assert!(result.is_ok());
        let saved = repo.find_by_id("B-001").unwrap();
        assert_eq!(*saved.get_status(), BookingStatus::Declined);
        assert!(saved.get_rooms().iter().all(|r| r.is_free()));
        assert!(!saved.blocks_slot());
        assert_eq!(notifier.sent(), vec![Notification::Declined("B-001".into())]);
    }

    #[test]
    fn unknown_booking_returns_not_found() {
        let mut repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = decline_booking("B-404", &responsible(), &mut repo, &clock, &notifier);

        assert!(matches!(result, Err(ApplicationError::BookingNotFound(id)) if id == "B-404"));
        assert!(notifier.sent().is_empty());
    }

    #[test]
    fn booking_older_than_24h_is_expired_and_saved_as_expired() {
        let mut repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(requested_at() + TimeDelta::hours(24));
        let notifier = FakeNotifier::default();

        let result = decline_booking("B-001", &responsible(), &mut repo, &clock, &notifier);

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::Booking(BookingError::Expired)))
        ));
        let saved = repo.find_by_id("B-001").unwrap();
        assert!(saved.is_expired());
        assert_eq!(*saved.get_status(), BookingStatus::Requested);
        assert!(notifier.sent().is_empty());
    }

    #[test]
    fn approved_booking_cannot_be_declined() {
        let mut repo = repo_with(BookingStatus::Approved);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = decline_booking("B-001", &responsible(), &mut repo, &clock, &notifier);

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::BookingAlreadyAccepted(_)))
        ));
        assert_eq!(*repo.find_by_id("B-001").unwrap().get_status(), BookingStatus::Approved);
        assert!(notifier.sent().is_empty());
    }

    #[test]
    fn declined_booking_cannot_be_declined_again() {
        let mut repo = repo_with(BookingStatus::Declined);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = decline_booking("B-001", &responsible(), &mut repo, &clock, &notifier);

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::BookingNotRequested(BookingStatus::Declined)))
        ));
        assert!(notifier.sent().is_empty());
    }
}
