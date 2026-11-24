#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, UserRepository},
        use_cases::common::{load_user, sort_chronologically},
    },
    domain::booking::Booking,
};

/// « Mes réservations » : toutes les réservations faites par l'appelant, quel que soit
/// leur statut, dans l'ordre chronologique de l'événement.
pub async fn list_my_bookings(
    user_id: &str,
    users: &impl UserRepository,
    repo: &impl BookingRepository,
) -> Result<Vec<Booking>, ApplicationError> {
    let user = load_user(users, user_id).await?;
    let mut bookings = repo.find_by_applicant(user.get_id()).await?;
    sort_chronologically(&mut bookings);
    Ok(bookings)
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{InMemoryBookingRepository, users},
            use_cases::list_my_bookings::list_my_bookings,
        },
        domain::{
            booking::{
                Booking, BookingStatus,
                tests::{booking, event_date, requested_at, salle_101, t},
            },
            user::{User, UserRole},
        },
    };

    fn booking_of_another_applicant() -> Booking {
        let applicant = User::new("A-002".into(), "User2".into(), UserRole::Applicant);
        Booking::new(
            "B-OTHER".into(), applicant, vec![salle_101()], BookingStatus::Approved, 10,
            event_date(), t(8, 0), t(9, 0), requested_at(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn returns_only_the_callers_bookings_in_chronological_order() {
        let repo = InMemoryBookingRepository::with(vec![
            booking("B-LATE", salle_101(), BookingStatus::Approved, t(14, 0), t(15, 0)),
            booking_of_another_applicant(),
            booking("B-EARLY", salle_101(), BookingStatus::Cancelled, t(10, 0), t(11, 0)),
        ]);

        let bookings = list_my_bookings("A-001", &users(), &repo).await.unwrap();

        let ids: Vec<&str> = bookings.iter().map(|b| b.get_id()).collect();
        assert_eq!(ids, vec!["B-EARLY", "B-LATE"]);
    }

    #[tokio::test]
    async fn no_booking_gives_an_empty_list() {
        let bookings = list_my_bookings("A-001", &users(), &InMemoryBookingRepository::default()).await.unwrap();

        assert!(bookings.is_empty());
    }

    #[tokio::test]
    async fn unknown_user_returns_user_not_found() {
        let result = list_my_bookings("U-404", &users(), &InMemoryBookingRepository::default()).await;

        assert!(matches!(result, Err(ApplicationError::UserNotFound(id)) if id == "U-404"));
    }
}
