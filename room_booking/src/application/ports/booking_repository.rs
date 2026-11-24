use chrono::NaiveDate;

use crate::{application::ports::RepositoryError, domain::booking::Booking};

/// Accès aux réservations enregistrées (implémenté par l'infrastructure : BDD, fichier…).
pub trait BookingRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Booking>, RepositoryError>;
    fn find_by_date(&self, date: NaiveDate) -> Result<Vec<Booking>, RepositoryError>;
    /// Réservations encore « vivantes » : en attente (non expirées) ou approuvées.
    /// Ce sont celles que les contrôles périodiques doivent surveiller.
    fn find_active(&self) -> Result<Vec<Booking>, RepositoryError>;
    fn save(&mut self, booking: Booking) -> Result<(), RepositoryError>;
}
