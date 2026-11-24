use crate::{
    application::{errors::ApplicationError, ports::BookingRepository},
    domain::booking::Booking,
};

/// Charge une réservation par son id.
pub fn load_booking(repo: &impl BookingRepository, booking_id: &str) -> Result<Booking, ApplicationError> {
    repo.find_by_id(booking_id)
        .ok_or_else(|| ApplicationError::BookingNotFound(booking_id.to_string()))
}

/// Charge une réservation et vérifie qu'elle appartient au demandeur.
pub fn load_owned_booking(
    repo: &impl BookingRepository,
    booking_id: &str,
    applicant_id: &str,
) -> Result<Booking, ApplicationError> {
    let booking = load_booking(repo, booking_id)?;
    if booking.get_applicant().get_id() != applicant_id {
        return Err(ApplicationError::NotBookingOwner {
            booking_id: booking_id.to_string(),
            applicant_id: applicant_id.to_string(),
        });
    }
    Ok(booking)
}
