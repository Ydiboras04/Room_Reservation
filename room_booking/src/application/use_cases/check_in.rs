#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, Clock},
        use_cases::common::load_owned_booking,
    },
    domain::{booking::Booking, errors::BookingError},
};

/// Use case « En attente de présence » → « Check-in reçu » → Salle utilisée :
/// 1. charge la réservation et vérifie que c'est bien son demandeur qui se présente ;
/// 2. le domaine enregistre le check-in (réservation approuvée, entre le début et début + 15 min) ;
/// 3. enregistre la réservation.
///
/// Si le check-in arrive trop tard, on applique tout de suite la règle
/// « Début + 15 min sans check-in » → Libérer la salle → Non présenté,
/// sans attendre le prochain contrôle périodique.
pub async fn check_in(
    booking_id: &str,
    applicant_id: &str,
    repo: &impl BookingRepository,
    clock: &impl Clock,
) -> Result<Booking, ApplicationError> {
    let mut booking = load_owned_booking(repo, booking_id, applicant_id).await?;
    let now = clock.now();

    match booking.check_in(now) {
        Ok(()) => {
            repo.save(booking.clone()).await?;
            Ok(booking)
        }
        Err(error @ BookingError::CheckInTooLate) => {
            booking.update_room_availability(now);
            repo.save(booking).await?;
            Err(error.into())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{FixedClock, InMemoryBookingRepository},
            ports::BookingRepository,
            use_cases::check_in::check_in,
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{booking, event_date, salle_101, t},
            },
            errors::BookingError,
        },
    };

    /// Réservation B-001 approuvée, de 10h à 12h, faite par A-001.
    fn repo_with_approved_booking() -> InMemoryBookingRepository {
        let mut b = booking("B-001", salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0));
        b.approve();
        InMemoryBookingRepository::with(vec![b])
    }

    #[tokio::test]
    async fn check_in_within_15_min_is_saved() {
        let repo = repo_with_approved_booking();
        let clock = FixedClock(event_date().and_time(t(10, 10)));

        let result = check_in("B-001", "A-001", &repo, &clock).await;

        assert!(result.is_ok());
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert!(saved.is_checked_in());
        assert!(saved.get_rooms().iter().all(|r| !r.is_free()));
    }

    #[tokio::test]
    async fn check_in_at_exactly_15_min_is_accepted() {
        let repo = repo_with_approved_booking();
        let clock = FixedClock(event_date().and_time(t(10, 15)));

        assert!(check_in("B-001", "A-001", &repo, &clock).await.is_ok());
    }

    #[tokio::test]
    async fn late_check_in_marks_no_show_and_releases_rooms() {
        let repo = repo_with_approved_booking();
        let clock = FixedClock(event_date().and_time(t(10, 16)));

        let result = check_in("B-001", "A-001", &repo, &clock).await;

        assert!(matches!(result, Err(ApplicationError::Booking(BookingError::CheckInTooLate))));
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert!(!saved.is_checked_in());
        assert_eq!(*saved.get_status(), BookingStatus::NoShow);
        assert!(saved.get_rooms().iter().all(|r| r.is_free()));
    }

    #[tokio::test]
    async fn early_check_in_is_refused_and_nothing_changes() {
        let repo = repo_with_approved_booking();
        let clock = FixedClock(event_date().and_time(t(9, 50)));

        let result = check_in("B-001", "A-001", &repo, &clock).await;

        assert!(matches!(result, Err(ApplicationError::Booking(BookingError::CheckInTooEarly))));
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert!(!saved.is_checked_in());
        assert_eq!(*saved.get_status(), BookingStatus::Approved);
    }

    #[tokio::test]
    async fn second_check_in_is_refused() {
        let repo = repo_with_approved_booking();
        check_in("B-001", "A-001", &repo, &FixedClock(event_date().and_time(t(10, 0)))).await.unwrap();

        let result = check_in("B-001", "A-001", &repo, &FixedClock(event_date().and_time(t(10, 5)))).await;

        assert!(matches!(result, Err(ApplicationError::Booking(BookingError::AlreadyCheckedIn))));
    }

    #[tokio::test]
    async fn check_in_on_pending_booking_is_refused() {
        let repo = InMemoryBookingRepository::default();
        repo.save(booking("B-001", salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0))).await.unwrap();
        let clock = FixedClock(event_date().and_time(t(10, 0)));

        let result = check_in("B-001", "A-001", &repo, &clock).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Booking(BookingError::CheckInNotApproved(BookingStatus::Requested)))
        ));
    }

    #[tokio::test]
    async fn another_applicant_cannot_check_in() {
        let repo = repo_with_approved_booking();
        let clock = FixedClock(event_date().and_time(t(10, 0)));

        let result = check_in("B-001", "A-999", &repo, &clock).await;

        assert!(matches!(result, Err(ApplicationError::NotBookingOwner { .. })));
        assert!(!repo.find_by_id("B-001").await.unwrap().unwrap().is_checked_in());
    }

    #[tokio::test]
    async fn unknown_booking_returns_not_found() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(event_date().and_time(t(10, 0)));

        let result = check_in("B-404", "A-001", &repo, &clock).await;

        assert!(matches!(result, Err(ApplicationError::BookingNotFound(id)) if id == "B-404"));
    }
}
