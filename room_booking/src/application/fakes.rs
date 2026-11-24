//! Faux adaptateurs pour tester les use cases sans infrastructure.
//! Les dépôts en mémoire vivent dans `infrastructure::in_memory` (ils servent aussi à
//! l'exécution) ; ils sont réexportés ici pour que les tests n'aient qu'un seul import.

use std::cell::RefCell;

use chrono::{NaiveDate, NaiveDateTime};

pub use crate::infrastructure::in_memory::{
    InMemoryBookingRepository, InMemoryRoomRepository, InMemoryUserRepository,
};
use crate::{
    application::ports::{BookingRepository, Clock, Notifier, RepositoryError, UserRepository},
    domain::{booking::Booking, system::TimeSlot, user::User},
};

/// Dépôt dont le stockage est en panne : chaque appel échoue.
/// Sert de dépôt de réservations comme de dépôt d'utilisateurs.
#[derive(Default)]
pub struct FailingRepository;

fn storage_down() -> RepositoryError {
    RepositoryError::storage("base de données injoignable")
}

impl BookingRepository for FailingRepository {
    fn find_by_id(&self, _id: &str) -> Result<Option<Booking>, RepositoryError> {
        Err(storage_down())
    }

    fn find_by_date(&self, _date: NaiveDate) -> Result<Vec<Booking>, RepositoryError> {
        Err(storage_down())
    }

    fn find_active(&self) -> Result<Vec<Booking>, RepositoryError> {
        Err(storage_down())
    }

    fn save(&mut self, _booking: Booking) -> Result<(), RepositoryError> {
        Err(storage_down())
    }
}

impl UserRepository for FailingRepository {
    fn find_by_id(&self, _id: &str) -> Result<Option<User>, RepositoryError> {
        Err(storage_down())
    }

    fn save(&mut self, _user: User) -> Result<(), RepositoryError> {
        Err(storage_down())
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

/// Utilisateurs de test : A-001 (demandeur) et RESP-001 (responsable).
pub fn users() -> InMemoryUserRepository {
    use crate::domain::user::UserRole;
    InMemoryUserRepository::with(vec![
        User::new("A-001".into(), "User1".into(), UserRole::Applicant),
        User::new("RESP-001".into(), "Chef".into(), UserRole::Responsible),
    ])
}
