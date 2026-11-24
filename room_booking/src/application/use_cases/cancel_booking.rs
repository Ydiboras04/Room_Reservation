#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, Clock, Notifier},
        use_cases::common::load_owned_booking,
    },
    domain::booking::Booking,
};

/// Sous-processus « Annulation » :
/// « Demande d'annulation » → « Vérifier le délai (R6) » → « ≥ 2 h avant ? »
///   - Oui → « Libérer le créneau et notifier » → Annulée
///   - Non → Annulation refusée
///
/// 1. charge la réservation ;
/// 2. vérifie que c'est bien le demandeur qui l'a faite qui l'annule ;
/// 3. le domaine applique R6 (≥ 2h avant) et libère les salles ;
/// 4. enregistre et notifie.
pub async fn cancel_booking(
    booking_id: &str,
    applicant_id: &str,
    repo: &impl BookingRepository,
    clock: &impl Clock,
    notifier: &impl Notifier,
) -> Result<Booking, ApplicationError> {
    let mut booking = load_owned_booking(repo, booking_id, applicant_id).await?;

    // En cas d'échec, `cancel` ne modifie pas la réservation : rien à enregistrer.
    booking.cancel(clock.now())?;

    repo.save(booking.clone()).await?;
    notifier.notify_cancelled(&booking).await;
    Ok(booking)
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{FakeNotifier, FixedClock, InMemoryBookingRepository, Notification},
            ports::BookingRepository,
            use_cases::cancel_booking::cancel_booking,
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{booking, event_date, salle_101, t},
            },
            errors::BookingError,
        },
    };

    /// Réservation B-001 de 10h à 12h, faite par A-001.
    fn repo_with(status: BookingStatus) -> InMemoryBookingRepository {
        let mut b = booking("B-001", salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0));
        if status == BookingStatus::Approved {
            b.approve(); // bloque réellement les salles
        } else if status != BookingStatus::Requested {
            b = booking("B-001", salle_101(), status, t(10, 0), t(12, 0));
        }
        InMemoryBookingRepository::with(vec![b])
    }

    #[tokio::test]
    async fn approved_booking_is_cancelled_rooms_released_and_notified() {
        let repo = repo_with(BookingStatus::Approved);
        let clock = FixedClock(event_date().and_time(t(7, 0)));
        let notifier = FakeNotifier::default();

        let result = cancel_booking("B-001", "A-001", &repo, &clock, &notifier).await;

        assert!(result.is_ok());
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert_eq!(*saved.get_status(), BookingStatus::Cancelled);
        assert!(saved.get_rooms().iter().all(|r| r.is_free()));
        assert!(!saved.blocks_slot());
        assert_eq!(notifier.sent(), vec![Notification::Cancelled("B-001".into())]);
    }

    #[tokio::test]
    async fn pending_booking_can_be_cancelled() {
        let repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(event_date().and_time(t(7, 0)));
        let notifier = FakeNotifier::default();

        let result = cancel_booking("B-001", "A-001", &repo, &clock, &notifier).await;

        assert!(result.is_ok());
        assert_eq!(*repo.find_by_id("B-001").await.unwrap().unwrap().get_status(), BookingStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancellation_exactly_2h_before_is_accepted() {
        let repo = repo_with(BookingStatus::Approved);
        let clock = FixedClock(event_date().and_time(t(8, 0)));
        let notifier = FakeNotifier::default();

        assert!(cancel_booking("B-001", "A-001", &repo, &clock, &notifier).await.is_ok());
    }

    #[tokio::test]
    async fn cancellation_less_than_2h_before_is_refused_and_nothing_changes() {
        // Réservation à 10h, annulation à 8h30 → R6
        let repo = repo_with(BookingStatus::Approved);
        let clock = FixedClock(event_date().and_time(t(8, 30)));
        let notifier = FakeNotifier::default();

        let result = cancel_booking("B-001", "A-001", &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::Booking(BookingError::AnnulationTooLate))));
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert_eq!(*saved.get_status(), BookingStatus::Approved);
        assert!(saved.get_rooms().iter().all(|r| !r.is_free()));
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn another_applicant_cannot_cancel() {
        let repo = repo_with(BookingStatus::Approved);
        let clock = FixedClock(event_date().and_time(t(7, 0)));
        let notifier = FakeNotifier::default();

        let result = cancel_booking("B-001", "A-999", &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::NotBookingOwner { booking_id, applicant_id })
                if booking_id == "B-001" && applicant_id == "A-999"
        ));
        assert_eq!(*repo.find_by_id("B-001").await.unwrap().unwrap().get_status(), BookingStatus::Approved);
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn declined_booking_cannot_be_cancelled() {
        let repo = repo_with(BookingStatus::Declined);
        let clock = FixedClock(event_date().and_time(t(7, 0)));
        let notifier = FakeNotifier::default();

        let result = cancel_booking("B-001", "A-001", &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Booking(BookingError::CannotCancel(BookingStatus::Declined)))
        ));
    }

    #[tokio::test]
    async fn unknown_booking_returns_not_found() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(event_date().and_time(t(7, 0)));
        let notifier = FakeNotifier::default();

        let result = cancel_booking("B-404", "A-001", &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::BookingNotFound(id)) if id == "B-404"));
    }
}
