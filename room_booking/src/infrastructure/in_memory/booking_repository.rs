use chrono::NaiveDate;

use crate::{application::ports::{BookingRepository, RepositoryError}, domain::booking::Booking};

/// Dépôt en mémoire : un simple `Vec`.
#[derive(Default)]
pub struct InMemoryBookingRepository {
    bookings: Vec<Booking>,
}

impl BookingRepository for InMemoryBookingRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Booking>, RepositoryError> {
        Ok(self.bookings.iter().find(|b| b.get_id() == id).cloned())
    }

    fn find_by_date(&self, date: NaiveDate) -> Result<Vec<Booking>, RepositoryError> {
        Ok(self.bookings.iter().filter(|b| b.get_event_date() == date).cloned().collect())
    }

    fn find_active(&self) -> Result<Vec<Booking>, RepositoryError> {
        Ok(self.bookings.iter().filter(|b| b.blocks_slot()).cloned().collect())
    }

    /// Insère ou remplace (même id).
    fn save(&mut self, booking: Booking) -> Result<(), RepositoryError> {
        match self.bookings.iter_mut().find(|b| b.get_id() == booking.get_id()) {
            Some(existing) => *existing = booking,
            None => self.bookings.push(booking),
        }
        Ok(())
    }
}
