#![allow(dead_code)]

use crate::domain::booking::BookingStatus;

#[derive(Debug, thiserror::Error)]
pub enum BookingError {
    #[error("l'heure de fin doit être supérieur à l'heure de début")]
    EventEndSmallerEventStart,

    #[error("la demande a expiré : aucune réponse dans les 72h")]
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
    AnnulationTooLate
}

#[derive(Debug, thiserror::Error)]
pub enum ApplicantError {
    #[error("L'utilisateur doit fournir un nom valide")]
    NameNotValid,
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
}