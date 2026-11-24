use std::sync::RwLock;

use chrono::NaiveDate;

use super::{read, upsert, write};
use crate::{
    application::ports::{BookingRepository, RepositoryError},
    domain::booking::Booking,
};

/// Dépôt de réservations en mémoire.
#[derive(Default)]
pub struct InMemoryBookingRepository {
    bookings: RwLock<Vec<Booking>>,
}

impl InMemoryBookingRepository {
    pub fn with(bookings: Vec<Booking>) -> Self {
        Self { bookings: RwLock::new(bookings) }
    }
}

impl BookingRepository for InMemoryBookingRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Booking>, RepositoryError> {
        Ok(read(&self.bookings)?.iter().find(|b| b.get_id() == id).cloned())
    }

    async fn find_by_date(&self, date: NaiveDate) -> Result<Vec<Booking>, RepositoryError> {
        Ok(read(&self.bookings)?.iter().filter(|b| b.get_event_date() == date).cloned().collect())
    }

    async fn find_active(&self) -> Result<Vec<Booking>, RepositoryError> {
        Ok(read(&self.bookings)?.iter().filter(|b| b.blocks_slot()).cloned().collect())
    }

    async fn save(&self, booking: Booking) -> Result<(), RepositoryError> {
        upsert(&mut *write(&self.bookings)?, booking, |a, b| a.get_id() == b.get_id());
        Ok(())
    }
}
