#![allow(dead_code)]

use crate::{
    application::ports::RepositoryError,
    domain::errors::{BookingError, ResponsibleError, SystemError, UserError},
};

/// Erreurs des use cases : celles du domaine, plus ce qui vient du chargement des données.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("aucune réservation trouvée avec l'id {0}")]
    BookingNotFound(String),

    #[error("aucune salle trouvée avec l'id {0}")]
    RoomNotFound(String),

    #[error("aucun utilisateur trouvé avec l'id {0}")]
    UserNotFound(String),

    #[error("le demandeur {applicant_id} n'est pas l'auteur de la réservation {booking_id}")]
    NotBookingOwner { booking_id: String, applicant_id: String },

    #[error(transparent)]
    Responsible(#[from] ResponsibleError),

    #[error(transparent)]
    User(#[from] UserError),

    #[error(transparent)]
    Booking(#[from] BookingError),

    #[error(transparent)]
    System(#[from] SystemError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
