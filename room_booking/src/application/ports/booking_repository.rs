use chrono::NaiveDate;

use crate::{application::ports::RepositoryError, domain::booking::Booking};

/// Accès aux réservations enregistrées (implémenté par l'infrastructure : BDD, fichier…).
///
/// Méthodes asynchrones (une vraie base répond via le réseau) et `&self` partout :
/// le dépôt est partagé entre requêtes concurrentes, comme un pool de connexions.
/// Les futures sont `Send` pour pouvoir tourner sur le runtime multi-thread de tokio.
/// Côté implémentation, on écrit simplement `async fn`.
pub trait BookingRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> impl Future<Output = Result<Option<Booking>, RepositoryError>> + Send;
    fn find_by_date(&self, date: NaiveDate) -> impl Future<Output = Result<Vec<Booking>, RepositoryError>> + Send;
    /// Réservations encore « vivantes » : en attente (non expirées) ou approuvées.
    /// Ce sont celles que les contrôles périodiques doivent surveiller.
    fn find_active(&self) -> impl Future<Output = Result<Vec<Booking>, RepositoryError>> + Send;
    fn save(&self, booking: Booking) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
