#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, UserRepository},
        use_cases::common::{load_user, sort_chronologically},
    },
    domain::{
        booking::{Booking, BookingStatus},
        user::UserRole,
    },
};

/// Vue d'ensemble pour les responsables : toutes les réservations, ou seulement celles
/// d'un statut (ex. `Requested` pour les demandes à valider), dans l'ordre chronologique.
///
/// Les demandes expirées (24 h sans réponse) restent au statut `Requested` avec
/// `is_expired() == true` : c'est à l'affichage de les distinguer.
pub async fn list_bookings(
    viewer_id: &str,
    status: Option<BookingStatus>,
    users: &impl UserRepository,
    repo: &impl BookingRepository,
) -> Result<Vec<Booking>, ApplicationError> {
    let viewer = load_user(users, viewer_id).await?;
    viewer.ensure_role(UserRole::Responsible)?;

    let mut bookings = match status {
        Some(status) => repo.find_by_status(status).await?,
        None => repo.find_all().await?,
    };
    sort_chronologically(&mut bookings);
    Ok(bookings)
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{InMemoryBookingRepository, users},
            use_cases::list_bookings::list_bookings,
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{amphi, booking, salle_101, t},
            },
            errors::UserError,
        },
    };

    fn repo() -> InMemoryBookingRepository {
        InMemoryBookingRepository::with(vec![
            booking("B-APPROVED", salle_101(), BookingStatus::Approved, t(8, 0), t(9, 0)),
            booking("B-PENDING-2", amphi(), BookingStatus::Requested, t(15, 0), t(16, 0)),
            booking("B-PENDING-1", amphi(), BookingStatus::Requested, t(10, 0), t(11, 0)),
        ])
    }

    #[tokio::test]
    async fn responsible_sees_requests_filtered_by_status() {
        let bookings = list_bookings("RESP-001", Some(BookingStatus::Requested), &users(), &repo()).await.unwrap();

        let ids: Vec<&str> = bookings.iter().map(|b| b.get_id()).collect();
        assert_eq!(ids, vec!["B-PENDING-1", "B-PENDING-2"]);
    }

    #[tokio::test]
    async fn without_filter_all_bookings_are_returned() {
        let bookings = list_bookings("RESP-001", None, &users(), &repo()).await.unwrap();

        assert_eq!(bookings.len(), 3);
    }

    #[tokio::test]
    async fn applicant_cannot_list_all_bookings() {
        let result = list_bookings("A-001", None, &users(), &repo()).await;

        assert!(matches!(
            result,
            Err(ApplicationError::User(UserError::MissingRole { user_id, .. })) if user_id == "A-001"
        ));
    }

    #[tokio::test]
    async fn unknown_viewer_returns_user_not_found() {
        let result = list_bookings("U-404", None, &users(), &repo()).await;

        assert!(matches!(result, Err(ApplicationError::UserNotFound(_))));
    }
}
