use crate::domain::{booking::Booking, system::TimeSlot};

/// Notifications envoyées aux acteurs du processus.
pub trait Notifier {
    /// Rappel 1h avant le début.
    fn send_reminder(&self, booking: &Booking);
    /// « Notifier l'erreur » → Rejetée (R1, R2, R4).
    fn notify_rejected(&self, booking_id: &str, reason: &str);
    /// « Proposer d'autres créneaux » → Indisponible (R3).
    fn notify_slot_unavailable(&self, booking_id: &str, alternatives: &[TimeSlot]);
    /// Salles standard : réservation confirmée.
    fn notify_confirmed(&self, booking: &Booking);
    /// Amphithéâtre : le responsable doit valider la demande.
    fn notify_pending_approval(&self, booking: &Booking);
    /// « Approuvée ? → Non » : le responsable a refusé la demande.
    fn notify_declined(&self, booking: &Booking);
    /// Sous-processus Annulation : « Libérer le créneau et notifier ».
    fn notify_cancelled(&self, booking: &Booking);
}
