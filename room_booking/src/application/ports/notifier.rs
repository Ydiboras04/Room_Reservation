use crate::domain::{booking::Booking, system::TimeSlot};

/// Notifications envoyées aux acteurs du processus.
/// Asynchrone : un vrai envoi (mail, SMS…) passe par le réseau.
pub trait Notifier: Send + Sync {
    /// Rappel 1h avant le début.
    fn send_reminder(&self, booking: &Booking) -> impl Future<Output = ()> + Send;
    /// « Notifier l'erreur » → Rejetée (R1, R2, R4).
    fn notify_rejected(&self, booking_id: &str, reason: &str) -> impl Future<Output = ()> + Send;
    /// « Proposer d'autres créneaux » → Indisponible (R3).
    fn notify_slot_unavailable(&self, booking_id: &str, alternatives: &[TimeSlot]) -> impl Future<Output = ()> + Send;
    /// Salles standard : réservation confirmée.
    fn notify_confirmed(&self, booking: &Booking) -> impl Future<Output = ()> + Send;
    /// Amphithéâtre : le responsable doit valider la demande.
    fn notify_pending_approval(&self, booking: &Booking) -> impl Future<Output = ()> + Send;
    /// « Approuvée ? → Non » : le responsable a refusé la demande.
    fn notify_declined(&self, booking: &Booking) -> impl Future<Output = ()> + Send;
    /// Sous-processus Annulation : « Libérer le créneau et notifier ».
    fn notify_cancelled(&self, booking: &Booking) -> impl Future<Output = ()> + Send;
}
