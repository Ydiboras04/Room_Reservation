//! Faux adaptateurs pour tester les use cases sans infrastructure.

use std::cell::RefCell;

use chrono::{NaiveDate, NaiveDateTime};

use crate::{
    application::ports::{BookingRepository, Clock, Notifier},
    domain::{booking::Booking, system::TimeSlot},
};

/// Dépôt en mémoire : un simple `Vec`.
#[derive(Default)]
pub struct InMemoryBookingRepository {
    bookings: Vec<Booking>,
}

impl BookingRepository for InMemoryBookingRepository {
    fn find_by_id(&self, id: &str) -> Option<Booking> {
        self.bookings.iter().find(|b| b.get_id() == id).cloned()
    }

    fn find_by_date(&self, date: NaiveDate) -> Vec<Booking> {
        self.bookings.iter().filter(|b| b.get_event_date() == date).cloned().collect()
    }

    fn find_active(&self) -> Vec<Booking> {
        self.bookings.iter().filter(|b| b.blocks_slot()).cloned().collect()
    }

    /// Insère ou remplace (même id).
    fn save(&mut self, booking: Booking) {
        match self.bookings.iter_mut().find(|b| b.get_id() == booking.get_id()) {
            Some(existing) => *existing = booking,
            None => self.bookings.push(booking),
        }
    }
}

/// Horloge figée à une date choisie par le test.
pub struct FixedClock(pub NaiveDateTime);

impl Clock for FixedClock {
    fn now(&self) -> NaiveDateTime {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    Reminder(String),
    Rejected(String, String),
    SlotUnavailable(String, Vec<TimeSlot>),
    Confirmed(String),
    PendingApproval(String),
    Declined(String),
    Cancelled(String),
}

/// Enregistre les notifications au lieu de les envoyer.
/// `RefCell` car les méthodes du trait prennent `&self`.
#[derive(Default)]
pub struct FakeNotifier {
    sent: RefCell<Vec<Notification>>,
}

impl FakeNotifier {
    pub fn sent(&self) -> Vec<Notification> {
        self.sent.borrow().clone()
    }
}

impl Notifier for FakeNotifier {
    fn send_reminder(&self, booking: &Booking) {
        self.sent.borrow_mut().push(Notification::Reminder(booking.get_id().into()));
    }

    fn notify_rejected(&self, booking_id: &str, reason: &str) {
        self.sent.borrow_mut().push(Notification::Rejected(booking_id.into(), reason.into()));
    }

    fn notify_slot_unavailable(&self, booking_id: &str, alternatives: &[TimeSlot]) {
        self.sent
            .borrow_mut()
            .push(Notification::SlotUnavailable(booking_id.into(), alternatives.to_vec()));
    }

    fn notify_confirmed(&self, booking: &Booking) {
        self.sent.borrow_mut().push(Notification::Confirmed(booking.get_id().into()));
    }

    fn notify_pending_approval(&self, booking: &Booking) {
        self.sent.borrow_mut().push(Notification::PendingApproval(booking.get_id().into()));
    }

    fn notify_declined(&self, booking: &Booking) {
        self.sent.borrow_mut().push(Notification::Declined(booking.get_id().into()));
    }

    fn notify_cancelled(&self, booking: &Booking) {
        self.sent.borrow_mut().push(Notification::Cancelled(booking.get_id().into()));
    }
}
