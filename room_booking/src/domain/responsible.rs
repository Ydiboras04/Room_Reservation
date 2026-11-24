#![allow(dead_code)]

use chrono::NaiveDateTime;

use crate::domain::{booking::{Booking, BookingStatus}, errors::ResponsibleError};

#[derive(Debug, Clone)]
pub struct Responsible {
    id: String,
    name: String
}

impl Responsible {
    pub fn new(id: String, name: String) -> Result<Self, ResponsibleError> {
        Ok(Self {
            id, 
            name 
        })
    }

    pub fn validate_request(&self, booking: &mut Booking, now: NaiveDateTime) -> Result<(), ResponsibleError> {
        booking.update_expiry(now)?;
        booking.ensure_not_expired()?;

        let status = *booking.get_status();
        if status == BookingStatus::Approved {
            return Err(ResponsibleError::BookingAlreadyAccepted(status));
        }

        if status != BookingStatus::Requested {
            return Err(ResponsibleError::BookingNotRequested(status));
        }

        booking.approve();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::domain::{
        booking::{
            BookingStatus,
            tests::{booking_with_status, requested_at},
        },
        errors::{BookingError, ResponsibleError},
        responsible::Responsible,
    };

    #[test]
    fn requested_booking_gets_approved() {
        let responsible = Responsible::new("RESP-001".into(), "Chef".into()).unwrap();
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = responsible.validate_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(result.is_ok());
        assert_eq!(*booking.get_status(), BookingStatus::Approved);
    }
    
    #[test]
    fn declined_booking_cannot_be_approved() {
        let responsible = Responsible::new("RESP-001".into(), "Chef".into()).unwrap();
        let mut booking = booking_with_status(BookingStatus::Declined);

        let result = responsible.validate_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(matches!(
            result,
            Err(ResponsibleError::BookingNotRequested(BookingStatus::Declined))
        ));
        assert_eq!(*booking.get_status(), BookingStatus::Declined);
    }

    #[test]
    fn approved_booking_cannot_be_approved_again() {
        let responsible = Responsible::new("RESP-001".into(), "Chef".into()).unwrap();
        let mut booking = booking_with_status(BookingStatus::Approved);

        let result = responsible.validate_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(matches!(
            result,
            Err(ResponsibleError::BookingAlreadyAccepted(BookingStatus::Approved))
        ));
        assert_eq!(*booking.get_status(), BookingStatus::Approved);
    }

    #[test]
    fn expired_booking_cannot_be_approved() {
        let responsible = Responsible::new("RESP-001".into(), "Chef".into()).unwrap();
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = responsible.validate_request(&mut booking, requested_at() + TimeDelta::hours(73));

        assert!(matches!(
            result,
            Err(ResponsibleError::Booking(BookingError::Expired))
        ));
        assert!(booking.is_expired());
        assert_eq!(*booking.get_status(), BookingStatus::Requested);
    }
}
