use crate::{
    application::ports::Notifier,
    domain::{booking::Booking, system::TimeSlot},
};

/// Notifier qui écrit dans la console. À remplacer plus tard par un envoi de mails.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConsoleNotifier;

impl ConsoleNotifier {
    fn print(&self, message: String) {
        println!("  📨 {message}");
    }
}

/// « 2026-12-01 10:00–12:00 »
fn describe(booking: &Booking) -> String {
    format!(
        "{} ({} {}–{})",
        booking.get_id(),
        booking.get_event_date(),
        booking.get_event_start().format("%H:%M"),
        booking.get_event_end().format("%H:%M"),
    )
}

impl Notifier for ConsoleNotifier {
    fn send_reminder(&self, booking: &Booking) {
        self.print(format!("Rappel : votre réservation {} commence dans moins d'1h", describe(booking)));
    }

    fn notify_rejected(&self, booking_id: &str, reason: &str) {
        self.print(format!("Demande {booking_id} rejetée : {reason}"));
    }

    fn notify_slot_unavailable(&self, booking_id: &str, alternatives: &[TimeSlot]) {
        let slots: Vec<String> = alternatives
            .iter()
            .map(|s| format!("{}–{}", s.start.format("%H:%M"), s.end.format("%H:%M")))
            .collect();
        if slots.is_empty() {
            self.print(format!("Demande {booking_id} : créneau indisponible, aucun autre créneau libre ce jour-là"));
        } else {
            self.print(format!("Demande {booking_id} : créneau indisponible. Créneaux libres : {}", slots.join(", ")));
        }
    }

    fn notify_confirmed(&self, booking: &Booking) {
        self.print(format!("Réservation {} confirmée", describe(booking)));
    }

    fn notify_pending_approval(&self, booking: &Booking) {
        self.print(format!("Réservation {} en attente de validation par le responsable", describe(booking)));
    }

    fn notify_declined(&self, booking: &Booking) {
        self.print(format!("Réservation {} refusée par le responsable", describe(booking)));
    }

    fn notify_cancelled(&self, booking: &Booking) {
        self.print(format!("Réservation {} annulée, créneau libéré", describe(booking)));
    }
}
