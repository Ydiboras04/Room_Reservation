#![allow(dead_code)]

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

    pub fn get_id(&self) -> &str {
        &self.id
    }
}