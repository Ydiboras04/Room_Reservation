#![allow(dead_code)]

use crate::domain::booking::Booking;
use crate::domain::errors::ApplicantError;

#[derive(Debug, Clone)]
pub struct Applicant {
    id: String,
    name: String
}

impl Applicant {
    pub fn new (id: String, name: String) -> Result<Self, ApplicantError> {
        Ok(Self{
            id, name
        })
    }

    pub fn submit_request (booking: Booking) -> Booking {
        return booking;
    }
}