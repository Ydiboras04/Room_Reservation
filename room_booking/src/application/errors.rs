#![allow(dead_code)]

use crate::domain::errors::{BookingError, ResponsibleError};

/// Erreurs des use cases : celles du domaine, plus ce qui vient du chargement des données.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("aucune réservation trouvée avec l'id {0}")]
    BookingNotFound(String),

    #[error("le demandeur {applicant_id} n'est pas l'auteur de la réservation {booking_id}")]
    NotBookingOwner { booking_id: String, applicant_id: String },

    #[error(transparent)]
    Responsible(#[from] ResponsibleError),

    #[error(transparent)]
    Booking(#[from] BookingError),
}
