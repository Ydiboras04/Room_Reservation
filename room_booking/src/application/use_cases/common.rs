use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, UserRepository},
    },
    domain::{booking::Booking, user::User},
};

/// Charge un utilisateur par son id. Le contrôle du rôle, s'il y en a un,
/// est fait par l'action du domaine elle-même (`User::ensure_role`).
pub async fn load_user(users: &impl UserRepository, user_id: &str) -> Result<User, ApplicationError> {
    users
        .find_by_id(user_id).await?
        .ok_or_else(|| ApplicationError::UserNotFound(user_id.to_string()))
}

/// Charge une réservation par son id.
pub async fn load_booking(repo: &impl BookingRepository, booking_id: &str) -> Result<Booking, ApplicationError> {
    repo.find_by_id(booking_id).await?
        .ok_or_else(|| ApplicationError::BookingNotFound(booking_id.to_string()))
}

/// Charge une réservation et vérifie qu'elle appartient au demandeur.
pub async fn load_owned_booking(
    repo: &impl BookingRepository,
    booking_id: &str,
    applicant_id: &str,
) -> Result<Booking, ApplicationError> {
    let booking = load_booking(repo, booking_id).await?;
    if booking.get_applicant().get_id() != applicant_id {
        return Err(ApplicationError::NotBookingOwner {
            booking_id: booking_id.to_string(),
            applicant_id: applicant_id.to_string(),
        });
    }
    Ok(booking)
}
