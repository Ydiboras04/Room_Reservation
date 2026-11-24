#![allow(dead_code)]

use crate::domain::{booking::BookingStatus, system::TimeSlot, user::UserRole};

#[derive(Debug, thiserror::Error)]
pub enum BookingError {
    #[error("l'heure de fin doit être supérieur à l'heure de début")]
    EventEndSmallerEventStart,

    #[error("la demande a expiré : aucune réponse dans les 24h")]
    Expired,

    #[error("la date actuelle ne peut pas être antérieure à la date de la demande")]
    NowBeforeRequestDate,

    #[error("la réservation doit être entre 7h et 19h, durée de 30mn à 4h")]
    ValideHoursAndDuration,

    #[error("le nombre de participant ne doit pas excédé la capacité de la salle")]
    ParticipantsSmallerThanRoomCapacity,

    #[error("le créneau ne doit chevaucher aucune réservation en attente ou confirmée")]
    OverlapReservation,

    #[error("la réervation doit se faire au moins 24h à l'avance et au plus 30 jours")]
    RequestTooLateOrTooEarly,

    #[error("Annulation possible seuelemnt 2h avant le début")]
    AnnulationTooLate,

    #[error("une réservation doit contenir au moins une salle")]
    NoRoomSelected,

    #[error("la salle {0} est présente plusieurs fois dans la réservation")]
    DuplicateRoom(String),

    #[error("le check-in n'est possible que pour une réservation approuvée (statut actuel : {0:?})")]
    CheckInNotApproved(BookingStatus),

    #[error("le check-in n'est pas encore ouvert : l'événement n'a pas commencé")]
    CheckInTooEarly,

    #[error("le check-in est fermé : plus de 15 min après le début de l'événement")]
    CheckInTooLate,

    #[error("le check-in a déjà été effectué")]
    AlreadyCheckedIn,

    #[error("seule une réservation en attente ou approuvée peut être annulée (statut actuel : {0:?})")]
    CannotCancel(BookingStatus),
}

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("l'utilisateur {user_id} n'a pas le rôle requis ({required:?})")]
    MissingRole { user_id: String, required: UserRole },
}

#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("la capacité doit être > 0")]
    ZeroCapactity,
}

#[derive(Debug, thiserror::Error)]
pub enum ResponsibleError {
    #[error("seule une demande en attente peut être validée (statut actuel : {0:?})")]
    BookingNotRequested(BookingStatus),

    #[error("cette demande a été déjà acceptée (status actuel : {0:?} )")]
    BookingAlreadyAccepted(BookingStatus),

    #[error(transparent)]
    Booking(#[from] BookingError),

    #[error(transparent)]
    User(#[from] UserError),
}

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    /// « Demande valide ? → Non » : R1, R2 ou R4 non respectée.
    #[error(transparent)]
    InvalidRequest(#[from] BookingError),

    /// « Créneau libre ? → Non » : conflit (R3), avec des créneaux proposés.
    #[error("le créneau n'est pas disponible ({} créneau(x) alternatif(s) proposé(s))", alternatives.len())]
    SlotUnavailable { alternatives: Vec<TimeSlot> },
}
