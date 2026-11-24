#![allow(dead_code)]

use chrono::NaiveDateTime;

use crate::domain::{
    booking::{Booking, BookingStatus},
    errors::{ResponsibleError, UserError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRole {
    /// Peut réserver des salles (couloir « Demandeur »).
    Applicant,
    /// Peut réserver, et valider ou refuser les demandes (couloir « Responsable des salles »).
    Responsible,
}

/// Un utilisateur de l'application : demandeur ou responsable selon son rôle.
/// Tout utilisateur peut réserver ; les actions réservées à un rôle
/// commencent par `ensure_role`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: String,
    name: String,
    role: UserRole,
}

impl User {
    pub fn new(id: String, name: String, role: UserRole) -> Self {
        Self { id, name, role }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_role(&self) -> UserRole {
        self.role
    }

    pub fn has_role(&self, role: UserRole) -> bool {
        self.role == role
    }

    /// Vérifie que l'utilisateur a le rôle requis. À appeler en tête de toute
    /// action réservée à un rôle, avant de modifier quoi que ce soit.
    pub fn ensure_role(&self, required: UserRole) -> Result<(), UserError> {
        if !self.has_role(required) {
            return Err(UserError::MissingRole {
                user_id: self.id.clone(),
                required,
            });
        }
        Ok(())
    }

    /// « Approuvée ? → Oui » : la demande (amphithéâtre) est validée et les salles bloquées.
    /// Réservé au responsable des salles.
    pub fn validate_request(&self, booking: &mut Booking, now: NaiveDateTime) -> Result<(), ResponsibleError> {
        self.ensure_role(UserRole::Responsible)?;
        Self::ensure_pending(booking, now)?;
        booking.approve();
        Ok(())
    }

    /// « Approuvée ? → Non » : la demande est refusée.
    /// Réservé au responsable des salles.
    pub fn decline_request(&self, booking: &mut Booking, now: NaiveDateTime) -> Result<(), ResponsibleError> {
        self.ensure_role(UserRole::Responsible)?;
        Self::ensure_pending(booking, now)?;
        booking.decline();
        Ok(())
    }

    /// La demande doit être en attente et ne pas avoir expiré (24h sans réponse).
    fn ensure_pending(booking: &mut Booking, now: NaiveDateTime) -> Result<(), ResponsibleError> {
        booking.update_expiry(now)?;
        booking.ensure_not_expired()?;

        let status = *booking.get_status();
        if status == BookingStatus::Approved {
            return Err(ResponsibleError::BookingAlreadyAccepted(status));
        }

        if status != BookingStatus::Requested {
            return Err(ResponsibleError::BookingNotRequested(status));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use chrono::TimeDelta;

    use crate::domain::{
        booking::{
            BookingStatus,
            tests::{booking_with_status, requested_at},
        },
        errors::{BookingError, ResponsibleError, UserError},
        user::{User, UserRole},
    };

    pub(crate) fn applicant() -> User {
        User::new("A-001".into(), "User1".into(), UserRole::Applicant)
    }

    pub(crate) fn responsible() -> User {
        User::new("RESP-001".into(), "Chef".into(), UserRole::Responsible)
    }

    // ---------- ensure_role ----------

    #[test]
    fn ensure_role_accepts_the_right_role() {
        assert!(responsible().ensure_role(UserRole::Responsible).is_ok());
        assert!(applicant().ensure_role(UserRole::Applicant).is_ok());
    }

    #[test]
    fn ensure_role_rejects_the_wrong_role() {
        let result = applicant().ensure_role(UserRole::Responsible);

        assert!(matches!(
            result,
            Err(UserError::MissingRole { user_id, required: UserRole::Responsible }) if user_id == "A-001"
        ));
    }

    #[test]
    fn applicant_cannot_validate_and_booking_is_untouched() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        // même après 24h : le rôle est vérifié avant l'expiration, rien n'est modifié
        let result = applicant().validate_request(&mut booking, requested_at() + TimeDelta::hours(25));

        assert!(matches!(result, Err(ResponsibleError::User(UserError::MissingRole { .. }))));
        assert_eq!(*booking.get_status(), BookingStatus::Requested);
        assert!(!booking.is_expired());
    }

    #[test]
    fn applicant_cannot_decline() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = applicant().decline_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(matches!(result, Err(ResponsibleError::User(UserError::MissingRole { .. }))));
        assert_eq!(*booking.get_status(), BookingStatus::Requested);
    }

    // ---------- validation / refus par le responsable ----------

    #[test]
    fn requested_booking_gets_approved() {
        let responsible = responsible();
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = responsible.validate_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(result.is_ok());
        assert_eq!(*booking.get_status(), BookingStatus::Approved);
    }
    
    #[test]
    fn declined_booking_cannot_be_approved() {
        let responsible = responsible();
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
        let responsible = responsible();
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
        let responsible = responsible();
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = responsible.validate_request(&mut booking, requested_at() + TimeDelta::hours(25));

        assert!(matches!(
            result,
            Err(ResponsibleError::Booking(BookingError::Expired))
        ));
        assert!(booking.is_expired());
        assert_eq!(*booking.get_status(), BookingStatus::Requested);
    }

    #[test]
    fn requested_booking_gets_declined() {
        let responsible = responsible();
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = responsible.decline_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(result.is_ok());
        assert_eq!(*booking.get_status(), BookingStatus::Declined);
    }

    #[test]
    fn approved_booking_cannot_be_declined() {
        let responsible = responsible();
        let mut booking = booking_with_status(BookingStatus::Approved);

        let result = responsible.decline_request(&mut booking, requested_at() + TimeDelta::hours(1));

        assert!(matches!(
            result,
            Err(ResponsibleError::BookingAlreadyAccepted(BookingStatus::Approved))
        ));
    }

    #[test]
    fn expired_booking_cannot_be_declined() {
        let responsible = responsible();
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = responsible.decline_request(&mut booking, requested_at() + TimeDelta::hours(24));

        assert!(matches!(result, Err(ResponsibleError::Booking(BookingError::Expired))));
    }
}
