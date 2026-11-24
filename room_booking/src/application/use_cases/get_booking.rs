#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, UserRepository},
        use_cases::common::{load_booking, load_user},
    },
    domain::{booking::Booking, user::UserRole},
};

/// Consultation d'une réservation : visible par son demandeur et par les responsables
/// (qui doivent pouvoir examiner une demande avant de la valider).
pub async fn get_booking(
    booking_id: &str,
    viewer_id: &str,
    users: &impl UserRepository,
    repo: &impl BookingRepository,
) -> Result<Booking, ApplicationError> {
    let viewer = load_user(users, viewer_id).await?;
    let booking = load_booking(repo, booking_id).await?;

    let is_owner = booking.get_applicant().get_id() == viewer.get_id();
    if !is_owner && viewer.get_role() != UserRole::Responsible {
        return Err(ApplicationError::NotBookingOwner {
            booking_id: booking_id.to_string(),
            applicant_id: viewer_id.to_string(),
        });
    }
    Ok(booking)
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{InMemoryBookingRepository, InMemoryUserRepository},
            use_cases::get_booking::get_booking,
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{booking, salle_101, t},
            },
            user::{User, UserRole},
        },
    };

    /// B-001 appartient à A-001 ; A-002 est un autre demandeur.
    fn setup() -> (InMemoryUserRepository, InMemoryBookingRepository) {
        let users = InMemoryUserRepository::with(vec![
            User::new("A-001".into(), "User1".into(), UserRole::Applicant),
            User::new("A-002".into(), "User2".into(), UserRole::Applicant),
            User::new("RESP-001".into(), "Chef".into(), UserRole::Responsible),
        ]);
        let repo = InMemoryBookingRepository::with(vec![booking(
            "B-001",
            salle_101(),
            BookingStatus::Requested,
            t(10, 0),
            t(12, 0),
        )]);
        (users, repo)
    }

    #[tokio::test]
    async fn owner_can_view_their_booking() {
        let (users, repo) = setup();

        let booking = get_booking("B-001", "A-001", &users, &repo).await.unwrap();

        assert_eq!(booking.get_id(), "B-001");
    }

    #[tokio::test]
    async fn responsible_can_view_any_booking() {
        let (users, repo) = setup();

        assert!(get_booking("B-001", "RESP-001", &users, &repo).await.is_ok());
    }

    #[tokio::test]
    async fn another_applicant_cannot_view_the_booking() {
        let (users, repo) = setup();

        let result = get_booking("B-001", "A-002", &users, &repo).await;

        assert!(matches!(result, Err(ApplicationError::NotBookingOwner { .. })));
    }

    #[tokio::test]
    async fn unknown_booking_returns_not_found() {
        let (users, repo) = setup();

        let result = get_booking("B-404", "A-001", &users, &repo).await;

        assert!(matches!(result, Err(ApplicationError::BookingNotFound(id)) if id == "B-404"));
    }

    #[tokio::test]
    async fn unknown_viewer_returns_user_not_found() {
        let (users, repo) = setup();

        let result = get_booking("B-001", "U-404", &users, &repo).await;

        assert!(matches!(result, Err(ApplicationError::UserNotFound(id)) if id == "U-404"));
    }
}
