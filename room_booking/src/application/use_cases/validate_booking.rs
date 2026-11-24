#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, Clock, Notifier, UserRepository},
        use_cases::common::{load_booking, load_user},
    },
    domain::booking::Booking,
};

/// Use case « Valider la demande (amphithéâtre) » → « Approuvée ? → Oui » :
/// 1. charge la réservation ;
/// 2. le responsable la valide (refusé si expirée après 24h, déjà acceptée, ou plus en attente) ;
/// 3. enregistre le nouvel état ;
/// 4. notifie le demandeur que la réservation est confirmée.
pub async fn validate_booking(
    booking_id: &str,
    responsible_id: &str,
    users: &impl UserRepository,
    repo: &impl BookingRepository,
    clock: &impl Clock,
    notifier: &impl Notifier,
) -> Result<Booking, ApplicationError> {
    let responsible = load_user(users, responsible_id).await?;
    let mut booking = load_booking(repo, booking_id).await?;

    let result = responsible.validate_request(&mut booking, clock.now());

    // Même en cas d'erreur, la réservation a pu changer (marquée expirée) :
    // on enregistre toujours son état pour ne pas le perdre.
    repo.save(booking.clone()).await?;

    result?;
    notifier.notify_confirmed(&booking).await;
    Ok(booking)
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{FakeNotifier, FixedClock, InMemoryBookingRepository, Notification, users},
            ports::BookingRepository,
            use_cases::validate_booking::validate_booking,
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{amphi, booking, requested_at, t},
            },
            errors::{BookingError, ResponsibleError, UserError},
        },
    };

    fn repo_with(status: BookingStatus) -> InMemoryBookingRepository {
        InMemoryBookingRepository::with(vec![booking("B-001", amphi(), status, t(10, 0), t(12, 0))])
    }

    #[tokio::test]
    async fn pending_booking_is_approved_saved_and_notified() {
        let repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-001", "RESP-001", &users(), &repo, &clock, &notifier).await;

        assert!(result.is_ok());
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert_eq!(*saved.get_status(), BookingStatus::Approved);
        assert!(saved.get_rooms().iter().all(|r| !r.is_free()));
        assert_eq!(notifier.sent(), vec![Notification::Confirmed("B-001".into())]);
    }

    #[tokio::test]
    async fn unknown_booking_returns_not_found() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-404", "RESP-001", &users(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::BookingNotFound(id)) if id == "B-404"));
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn booking_older_than_24h_is_expired_and_saved_as_expired() {
        let repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(requested_at() + TimeDelta::hours(24));
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-001", "RESP-001", &users(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::Booking(BookingError::Expired)))
        ));
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert!(saved.is_expired());
        assert_eq!(*saved.get_status(), BookingStatus::Requested);
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn already_approved_booking_is_rejected() {
        let repo = repo_with(BookingStatus::Approved);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-001", "RESP-001", &users(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::BookingAlreadyAccepted(_)))
        ));
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn declined_booking_cannot_be_validated() {
        let repo = repo_with(BookingStatus::Declined);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-001", "RESP-001", &users(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::BookingNotRequested(BookingStatus::Declined)))
        ));
        assert_eq!(*repo.find_by_id("B-001").await.unwrap().unwrap().get_status(), BookingStatus::Declined);
    }

    #[tokio::test]
    async fn applicant_cannot_act_as_responsible() {
        let repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-001", "A-001", &users(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::Responsible(ResponsibleError::User(UserError::MissingRole { user_id, .. })))
                if user_id == "A-001"
        ));
        assert_eq!(*repo.find_by_id("B-001").await.unwrap().unwrap().get_status(), BookingStatus::Requested);
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn unknown_responsible_returns_user_not_found() {
        let repo = repo_with(BookingStatus::Requested);
        let clock = FixedClock(requested_at() + TimeDelta::hours(1));
        let notifier = FakeNotifier::default();

        let result = validate_booking("B-001", "U-404", &users(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::UserNotFound(id)) if id == "U-404"));
    }
}
