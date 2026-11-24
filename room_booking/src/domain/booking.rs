#![allow(dead_code)]

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike};

use crate::domain::applicant::Applicant;
use crate::domain::room::{Room, RoomType};
use crate::domain::errors::BookingError;

// Expiration : une demande sans réponse du responsable expire après 24h
pub const EXPIRY_DELAY_HOURS: i64 = 24;

// R1 : créneau valide
pub const OPENING_HOUR: u32 = 7;
pub const CLOSING_HOUR: u32 = 19;
pub const MIN_DURATION_MINUTES: i64 = 30;
pub const MAX_DURATION_MINUTES: i64 = 4 * 60;
pub const SLOT_STEP_MINUTES: i64 = 30;

// R4 : délai de réservation
pub const MIN_LEAD_HOURS: i64 = 24;
pub const MAX_LEAD_DAYS: i64 = 30;

// R6 : délai d'annulation
pub const MIN_CANCELLATION_HOURS: i64 = 2;

// Check-in : au-delà de 15 min après le début sans check-in, les salles sont libérées
pub const CHECK_IN_GRACE_MINUTES: i64 = 15;

// Rappel envoyé au demandeur 1h avant le début
pub const REMINDER_BEFORE_HOURS: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingStatus {
    Requested,
    Approved,
    Declined,
    Cancelled,
    NoShow,
}

#[derive(Debug, Clone)]
pub struct Booking {
    id: String,
    applicant: Applicant,
    rooms: Vec<Room>,
    status: BookingStatus,
    participants: u8,
    event_date: NaiveDate,
    event_start: NaiveTime,
    event_end: NaiveTime,
    expired: bool,
    checked_in: bool,
    reminder_sent: bool,
    requested_at: NaiveDateTime,
}

impl Booking {
    pub fn new (id: String, applicant: Applicant, rooms: Vec<Room>, status: BookingStatus, participants: u8, event_date: NaiveDate, event_start: NaiveTime, event_end: NaiveTime, requested_at: NaiveDateTime) -> Result<Self, BookingError> {
        Self::check_rooms(&rooms)?;
        Self::check_slot(event_start, event_end)?;                       // R1
        Self::check_capacity(&rooms, participants)?;                     // R2
        Self::check_lead_time(event_date.and_time(event_start), requested_at)?; // R4

        Ok(Self{
            id, applicant, rooms, status, participants, event_date, event_start, event_end,
            expired: false, checked_in: false, reminder_sent: false, requested_at
        })
    }

    /// Au moins une salle, et pas deux fois la même.
    fn check_rooms(rooms: &[Room]) -> Result<(), BookingError> {
        if rooms.is_empty() {
            return Err(BookingError::NoRoomSelected);
        }
        for (i, room) in rooms.iter().enumerate() {
            if rooms[..i].iter().any(|other| other.get_id() == room.get_id()) {
                return Err(BookingError::DuplicateRoom(room.get_id().to_string()));
            }
        }
        Ok(())
    }

    /// R1 : entre 7h et 19h, durée de 30 min à 4h, par pas de 30 min.
    fn check_slot(start: NaiveTime, end: NaiveTime) -> Result<(), BookingError> {
        if end < start {
            return Err(BookingError::EventEndSmallerEventStart);
        }

        let opening = NaiveTime::from_hms_opt(OPENING_HOUR, 0, 0).unwrap();
        let closing = NaiveTime::from_hms_opt(CLOSING_HOUR, 0, 0).unwrap();
        let duration = (end - start).num_minutes();
        let starts_on_step = start.minute() as i64 % SLOT_STEP_MINUTES == 0 && start.second() == 0;

        if start < opening
            || end > closing
            || duration < MIN_DURATION_MINUTES
            || duration > MAX_DURATION_MINUTES
            || duration % SLOT_STEP_MINUTES != 0
            || !starts_on_step
        {
            return Err(BookingError::ValideHoursAndDuration);
        }
        Ok(())
    }

    /// R2 : nombre de participants ≤ capacité totale des salles réservées.
    fn check_capacity(rooms: &[Room], participants: u8) -> Result<(), BookingError> {
        let total: u32 = rooms.iter().map(|room| room.get_capacity() as u32).sum();
        if participants as u32 > total {
            return Err(BookingError::ParticipantsSmallerThanRoomCapacity);
        }
        Ok(())
    }

    /// R4 : au moins 24h à l'avance et au plus 30 jours.
    fn check_lead_time(event_start_at: NaiveDateTime, requested_at: NaiveDateTime) -> Result<(), BookingError> {
        let lead = event_start_at - requested_at;
        if lead < TimeDelta::hours(MIN_LEAD_HOURS) || lead > TimeDelta::days(MAX_LEAD_DAYS) {
            return Err(BookingError::RequestTooLateOrTooEarly);
        }
        Ok(())
    }

    /// Privé : on passe par `approve`, `decline`, `cancel`… pour garder
    /// le statut et la disponibilité des salles cohérents.
    fn set_status(&mut self, status: BookingStatus) {
        self.status = status;
    }

    /// Même demande sur un autre créneau, revalidée (R1, R2, R4).
    /// Sert à proposer des créneaux alternatifs.
    pub fn with_slot(&self, start: NaiveTime, end: NaiveTime) -> Result<Booking, BookingError> {
        Booking::new(
            self.id.clone(),
            self.applicant.clone(),
            self.rooms.clone(),
            self.status,
            self.participants,
            self.event_date,
            start,
            end,
            self.requested_at,
        )
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_applicant(&self) -> &Applicant {
        &self.applicant
    }

    pub fn get_event_date(&self) -> NaiveDate {
        self.event_date
    }

    pub fn get_event_start(&self) -> NaiveTime {
        self.event_start
    }

    pub fn get_event_end(&self) -> NaiveTime {
        self.event_end
    }

    pub fn get_status(&self) -> &BookingStatus {
        &self.status
    }

    pub fn get_rooms(&self) -> &[Room] {
        &self.rooms
    }

    pub fn is_expired(&self) -> bool {
        self.expired
    }

    pub fn is_checked_in(&self) -> bool {
        self.checked_in
    }

    pub fn event_start_at(&self) -> NaiveDateTime {
        self.event_date.and_time(self.event_start)
    }

    pub fn event_end_at(&self) -> NaiveDateTime {
        self.event_date.and_time(self.event_end)
    }

    pub fn update_expiry(&mut self, now: NaiveDateTime) -> Result<(), BookingError> {
        if now < self.requested_at {
            return Err(BookingError::NowBeforeRequestDate);
        }

        if self.status == BookingStatus::Requested
            && now - self.requested_at >= TimeDelta::hours(EXPIRY_DELAY_HOURS)
        {
            self.expired = true;
        }
        Ok(())
    }

    /// Approuve la réservation et marque toutes ses salles comme occupées.
    pub fn approve(&mut self) {
        self.set_status(BookingStatus::Approved);
        self.rooms.iter_mut().for_each(Room::occupy);
    }

    /// Refusée par le responsable.
    pub fn decline(&mut self) {
        self.set_status(BookingStatus::Declined);
        self.release_rooms();
    }

    /// R5 : approuvée automatiquement seulement si toutes les salles sont standard.
    pub fn auto_validate(&mut self) {
        if self.rooms.iter().all(|room| room.get_kind() == RoomType::Standard) {
            self.approve();
        }
    }

    fn release_rooms(&mut self) {
        self.rooms.iter_mut().for_each(Room::release);
    }

    pub fn ensure_not_expired(&self) -> Result<(), BookingError> {
        if self.expired {
            return Err(BookingError::Expired);
        }
        Ok(())
    }

    /// Une réservation bloque le créneau si elle est en attente (non expirée) ou confirmée.
    pub fn blocks_slot(&self) -> bool {
        match self.status {
            BookingStatus::Requested => !self.expired,
            BookingStatus::Approved => true,
            BookingStatus::Declined | BookingStatus::Cancelled | BookingStatus::NoShow => false,
        }
    }

    /// Au moins une salle en commun, même jour, et les créneaux se chevauchent.
    /// 08:00–10:00 et 10:00–11:00 ne se chevauchent pas.
    pub fn overlaps(&self, other: &Booking) -> bool {
        let shares_a_room = self
            .rooms
            .iter()
            .any(|room| other.rooms.iter().any(|o| o.get_id() == room.get_id()));

        self.id != other.id
            && shares_a_room
            && self.event_date == other.event_date
            && self.event_start < other.event_end
            && other.event_start < self.event_end
    }

    /// R3 : le créneau ne doit chevaucher aucune réservation en attente ou confirmée.
    /// Les réservations existantes sont fournies par l'appelant (couche application / dépôt).
    pub fn ensure_no_conflict(&self, existing: &[Booking]) -> Result<(), BookingError> {
        if existing.iter().any(|other| other.blocks_slot() && self.overlaps(other)) {
            return Err(BookingError::OverlapReservation);
        }
        Ok(())
    }

    /// R6 : annulation possible seulement jusqu'à 2h avant le début.
    /// Seule une réservation en attente ou approuvée peut être annulée.
    pub fn cancel(&mut self, now: NaiveDateTime) -> Result<(), BookingError> {
        if !matches!(self.status, BookingStatus::Requested | BookingStatus::Approved) {
            return Err(BookingError::CannotCancel(self.status));
        }
        if now > self.event_start_at() - TimeDelta::hours(MIN_CANCELLATION_HOURS) {
            return Err(BookingError::AnnulationTooLate);
        }
        self.set_status(BookingStatus::Cancelled);
        self.release_rooms();
        Ok(())
    }

    /// Check-in du demandeur : possible entre le début de l'événement et 15 min après,
    /// uniquement pour une réservation approuvée.
    pub fn check_in(&mut self, now: NaiveDateTime) -> Result<(), BookingError> {
        if self.checked_in {
            return Err(BookingError::AlreadyCheckedIn);
        }
        if self.status != BookingStatus::Approved {
            return Err(BookingError::CheckInNotApproved(self.status));
        }
        if now < self.event_start_at() {
            return Err(BookingError::CheckInTooEarly);
        }
        if now > self.event_start_at() + TimeDelta::minutes(CHECK_IN_GRACE_MINUTES) {
            return Err(BookingError::CheckInTooLate);
        }
        self.checked_in = true;
        Ok(())
    }

    /// Le rappel est dû à partir d'1h avant le début, pour une réservation
    /// approuvée, tant qu'il n'a pas été envoyé et que l'événement n'a pas commencé.
    pub fn reminder_due(&self, now: NaiveDateTime) -> bool {
        self.status == BookingStatus::Approved
            && !self.reminder_sent
            && now >= self.event_start_at() - TimeDelta::hours(REMINDER_BEFORE_HOURS)
            && now < self.event_start_at()
    }

    /// À appeler par l'infrastructure une fois le rappel réellement envoyé.
    pub fn mark_reminder_sent(&mut self) {
        self.reminder_sent = true;
    }

    pub fn is_reminder_sent(&self) -> bool {
        self.reminder_sent
    }

    /// Met à jour la disponibilité des salles selon l'heure actuelle :
    /// - événement terminé → salles libérées ;
    /// - 15 min après le début sans check-in → `NoShow`, salles libérées.
    pub fn update_room_availability(&mut self, now: NaiveDateTime) {
        if self.status != BookingStatus::Approved {
            return;
        }

        if now >= self.event_end_at() {
            self.release_rooms();
        } else if !self.checked_in
            && now > self.event_start_at() + TimeDelta::minutes(CHECK_IN_GRACE_MINUTES)
        {
            self.set_status(BookingStatus::NoShow);
            self.release_rooms();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

    use crate::domain::{
        applicant::Applicant,
        booking::{Booking, BookingStatus},
        errors::BookingError,
        room::{Room, RoomType},
    };

    // ---------- helpers ----------

    pub(crate) fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    pub(crate) fn event_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()
    }

    pub(crate) fn requested_at() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 25).unwrap().and_hms_opt(9, 0, 0).unwrap()
    }

    pub(crate) fn amphi() -> Room {
        Room::new("R-001".into(), "Amphi A".into(), 100, RoomType::Amphi, true).unwrap()
    }

    pub(crate) fn salle_101() -> Room {
        Room::new("R-101".into(), "Salle 101".into(), 30, RoomType::Standard, true).unwrap()
    }

    /// Construit une réservation où seul le créneau change.
    fn try_slot(start: NaiveTime, end: NaiveTime) -> Result<Booking, BookingError> {
        Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![amphi()],
            BookingStatus::Requested,
            10,
            event_date(),
            start,
            end,
            requested_at(),
        )
    }

    pub(crate) fn booking(id: &str, room: Room, status: BookingStatus, start: NaiveTime, end: NaiveTime) -> Booking {
        Booking::new(
            id.into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![room],
            status,
            10,
            event_date(),
            start,
            end,
            requested_at(),
        )
        .unwrap()
    }

    pub(crate) fn booking_with_status(status: BookingStatus) -> Booking {
        booking("B-001", amphi(), status, t(10, 0), t(12, 0))
    }

    // ---------- R1 : créneau valide ----------

    #[test]
    fn event_end_smaller_than_event_start() {
        let result = try_slot(t(14, 0), t(10, 0));
        assert!(matches!(result, Err(BookingError::EventEndSmallerEventStart)));
    }

    #[test]
    fn r1_duration_not_multiple_of_30_min_is_rejected() {
        let result = try_slot(t(8, 0), t(8, 45));
        assert!(matches!(result, Err(BookingError::ValideHoursAndDuration)));
    }

    #[test]
    fn r1_start_before_7h_is_rejected() {
        let result = try_slot(t(6, 30), t(8, 0));
        assert!(matches!(result, Err(BookingError::ValideHoursAndDuration)));
    }

    #[test]
    fn r1_end_after_19h_is_rejected() {
        let result = try_slot(t(18, 0), t(19, 30));
        assert!(matches!(result, Err(BookingError::ValideHoursAndDuration)));
    }

    #[test]
    fn r1_duration_over_4h_is_rejected() {
        let result = try_slot(t(8, 0), t(13, 0));
        assert!(matches!(result, Err(BookingError::ValideHoursAndDuration)));
    }

    #[test]
    fn r1_start_not_on_30_min_step_is_rejected() {
        let result = try_slot(t(8, 15), t(9, 15));
        assert!(matches!(result, Err(BookingError::ValideHoursAndDuration)));
    }

    #[test]
    fn r1_limits_are_accepted() {
        assert!(try_slot(t(7, 0), t(7, 30)).is_ok());   // 30 min dès l'ouverture
        assert!(try_slot(t(15, 0), t(19, 0)).is_ok());  // 4h jusqu'à la fermeture
    }

    // ---------- R2 : capacité ----------

    #[test]
    fn r2_too_many_participants_is_rejected() {
        let result = Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![salle_101()],
            BookingStatus::Requested,
            40,
            event_date(),
            t(10, 0),
            t(11, 0),
            requested_at(),
        );
        assert!(matches!(result, Err(BookingError::ParticipantsSmallerThanRoomCapacity)));
    }

    #[test]
    fn r2_participants_equal_to_capacity_is_accepted() {
        let result = Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![salle_101()],
            BookingStatus::Requested,
            30,
            event_date(),
            t(10, 0),
            t(11, 0),
            requested_at(),
        );
        assert!(result.is_ok());
    }

    // ---------- R3 : conflit ----------

    #[test]
    fn r3_overlapping_confirmed_booking_is_rejected() {
        let existing = vec![booking("B-001", amphi(), BookingStatus::Approved, t(8, 0), t(10, 0))];
        let new = booking("B-002", amphi(), BookingStatus::Requested, t(9, 30), t(10, 30));

        let result = new.ensure_no_conflict(&existing);

        assert!(matches!(result, Err(BookingError::OverlapReservation)));
    }

    #[test]
    fn r3_overlapping_pending_booking_is_rejected() {
        let existing = vec![booking("B-001", amphi(), BookingStatus::Requested, t(8, 0), t(10, 0))];
        let new = booking("B-002", amphi(), BookingStatus::Requested, t(9, 30), t(10, 30));

        assert!(matches!(new.ensure_no_conflict(&existing), Err(BookingError::OverlapReservation)));
    }

    #[test]
    fn r3_adjacent_slot_is_accepted() {
        let existing = vec![booking("B-001", amphi(), BookingStatus::Approved, t(8, 0), t(10, 0))];
        let new = booking("B-002", amphi(), BookingStatus::Requested, t(10, 0), t(11, 0));

        assert!(new.ensure_no_conflict(&existing).is_ok());
    }

    #[test]
    fn r3_declined_or_cancelled_booking_does_not_block() {
        let existing = vec![
            booking("B-001", amphi(), BookingStatus::Declined, t(8, 0), t(10, 0)),
            booking("B-002", amphi(), BookingStatus::Cancelled, t(8, 0), t(10, 0)),
        ];
        let new = booking("B-003", amphi(), BookingStatus::Requested, t(9, 30), t(10, 30));

        assert!(new.ensure_no_conflict(&existing).is_ok());
    }

    #[test]
    fn r3_expired_booking_does_not_block() {
        let mut old = booking("B-001", amphi(), BookingStatus::Requested, t(8, 0), t(10, 0));
        old.update_expiry(requested_at() + TimeDelta::hours(24)).unwrap();
        let new = booking("B-002", amphi(), BookingStatus::Requested, t(9, 30), t(10, 30));

        assert!(new.ensure_no_conflict(&[old]).is_ok());
    }

    #[test]
    fn r3_same_slot_in_another_room_is_accepted() {
        let existing = vec![booking("B-001", amphi(), BookingStatus::Approved, t(8, 0), t(10, 0))];
        let new = booking("B-002", salle_101(), BookingStatus::Requested, t(9, 30), t(10, 30));

        assert!(new.ensure_no_conflict(&existing).is_ok());
    }

    // ---------- R4 : délai ----------

    #[test]
    fn r4_less_than_24h_in_advance_is_rejected() {
        // Demande lundi 30/11 à 15h pour mardi 01/12 à 10h → 19h d'avance
        let requested = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap().and_hms_opt(15, 0, 0).unwrap();
        let result = Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![amphi()],
            BookingStatus::Requested,
            10,
            event_date(),
            t(10, 0),
            t(11, 0),
            requested,
        );
        assert!(matches!(result, Err(BookingError::RequestTooLateOrTooEarly)));
    }

    #[test]
    fn r4_more_than_30_days_in_advance_is_rejected() {
        let requested = event_date().and_time(t(10, 0)) - TimeDelta::days(30) - TimeDelta::minutes(1);
        let result = Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![amphi()],
            BookingStatus::Requested,
            10,
            event_date(),
            t(10, 0),
            t(11, 0),
            requested,
        );
        assert!(matches!(result, Err(BookingError::RequestTooLateOrTooEarly)));
    }

    #[test]
    fn r4_exactly_24h_in_advance_is_accepted() {
        let requested = event_date().and_time(t(10, 0)) - TimeDelta::hours(24);
        let result = Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            vec![amphi()],
            BookingStatus::Requested,
            10,
            event_date(),
            t(10, 0),
            t(11, 0),
            requested,
        );
        assert!(result.is_ok());
    }

    // ---------- R6 : annulation ----------

    #[test]
    fn r6_cancellation_less_than_2h_before_is_rejected() {
        // Réservation à 10h, annulation à 8h30
        let mut b = booking_with_status(BookingStatus::Approved);
        let now = event_date().and_time(t(8, 30));

        let result = b.cancel(now);

        assert!(matches!(result, Err(BookingError::AnnulationTooLate)));
        assert_eq!(*b.get_status(), BookingStatus::Approved);
    }

    #[test]
    fn r6_cancellation_exactly_2h_before_is_accepted() {
        let mut b = booking_with_status(BookingStatus::Approved);
        let now = event_date().and_time(t(8, 0));

        assert!(b.cancel(now).is_ok());
        assert_eq!(*b.get_status(), BookingStatus::Cancelled);
    }

    // ---------- expiration (24h) ----------

    #[test]
    fn requested_booking_expires_after_24h() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        booking.update_expiry(requested_at() + TimeDelta::hours(24)).unwrap();

        assert!(booking.is_expired());
        assert!(matches!(booking.ensure_not_expired(), Err(BookingError::Expired)));
    }

    #[test]
    fn requested_booking_does_not_expire_before_24h() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        booking
            .update_expiry(requested_at() + TimeDelta::hours(24) - TimeDelta::seconds(1))
            .unwrap();

        assert!(!booking.is_expired());
        assert!(booking.ensure_not_expired().is_ok());
    }

    #[test]
    fn approved_booking_never_expires() {
        let mut booking = booking_with_status(BookingStatus::Approved);

        booking.update_expiry(requested_at() + TimeDelta::days(10)).unwrap();

        assert!(!booking.is_expired());
    }

    #[test]
    fn update_expiry_rejects_now_before_request_date() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        let result = booking.update_expiry(requested_at() - TimeDelta::hours(1));

        assert!(matches!(result, Err(BookingError::NowBeforeRequestDate)));
        assert!(!booking.is_expired());
    }

    // ---------- plusieurs salles ----------

    fn try_rooms(rooms: Vec<Room>, participants: u8) -> Result<Booking, BookingError> {
        Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            rooms,
            BookingStatus::Requested,
            participants,
            event_date(),
            t(10, 0),
            t(12, 0),
            requested_at(),
        )
    }

    #[test]
    fn booking_can_hold_several_rooms() {
        let b = try_rooms(vec![amphi(), salle_101()], 10).unwrap();
        assert_eq!(b.get_rooms().len(), 2);
    }

    #[test]
    fn booking_without_room_is_rejected() {
        assert!(matches!(try_rooms(vec![], 10), Err(BookingError::NoRoomSelected)));
    }

    #[test]
    fn booking_with_same_room_twice_is_rejected() {
        let result = try_rooms(vec![salle_101(), salle_101()], 10);
        assert!(matches!(result, Err(BookingError::DuplicateRoom(id)) if id == "R-101"));
    }

    #[test]
    fn r2_capacity_is_the_sum_of_all_rooms() {
        // 100 (amphi) + 30 (salle 101) = 130 places
        assert!(try_rooms(vec![amphi(), salle_101()], 130).is_ok());
        assert!(matches!(
            try_rooms(vec![amphi(), salle_101()], 131),
            Err(BookingError::ParticipantsSmallerThanRoomCapacity)
        ));
    }

    #[test]
    fn r3_conflict_if_one_room_is_shared() {
        let existing = vec![booking("B-000", salle_101(), BookingStatus::Approved, t(10, 0), t(12, 0))];
        let new = try_rooms(vec![amphi(), salle_101()], 10).unwrap();

        assert!(matches!(new.ensure_no_conflict(&existing), Err(BookingError::OverlapReservation)));
    }

    #[test]
    fn r5_auto_validate_only_if_all_rooms_are_standard() {
        let mut with_amphi = try_rooms(vec![amphi(), salle_101()], 10).unwrap();
        with_amphi.auto_validate();
        assert_eq!(*with_amphi.get_status(), BookingStatus::Requested);

        let mut standard_only = try_rooms(vec![salle_101()], 10).unwrap();
        standard_only.auto_validate();
        assert_eq!(*standard_only.get_status(), BookingStatus::Approved);
    }

    // ---------- disponibilité des salles ----------

    fn approved_booking() -> Booking {
        let mut b = try_rooms(vec![amphi(), salle_101()], 10).unwrap();
        b.approve();
        b
    }

    fn all_free(b: &Booking) -> bool {
        b.get_rooms().iter().all(|r| r.is_free())
    }

    fn none_free(b: &Booking) -> bool {
        b.get_rooms().iter().all(|r| !r.is_free())
    }

    #[test]
    fn approving_marks_all_rooms_as_occupied() {
        let b = approved_booking();
        assert!(none_free(&b));
    }

    #[test]
    fn rooms_are_released_when_event_is_over() {
        let mut b = approved_booking();
        b.check_in(event_date().and_time(t(10, 5))).unwrap();

        b.update_room_availability(event_date().and_time(t(12, 0)));

        assert!(all_free(&b));
        assert_eq!(*b.get_status(), BookingStatus::Approved);
    }

    #[test]
    fn rooms_stay_occupied_during_event_after_check_in() {
        let mut b = approved_booking();
        b.check_in(event_date().and_time(t(10, 5))).unwrap();

        b.update_room_availability(event_date().and_time(t(11, 0)));

        assert!(none_free(&b));
    }

    #[test]
    fn no_check_in_after_15_min_releases_rooms() {
        let mut b = approved_booking();

        b.update_room_availability(event_date().and_time(t(10, 16)));

        assert!(all_free(&b));
        assert_eq!(*b.get_status(), BookingStatus::NoShow);
        assert!(!b.is_checked_in());
    }

    #[test]
    fn rooms_stay_occupied_within_15_min_grace_period() {
        let mut b = approved_booking();

        b.update_room_availability(event_date().and_time(t(10, 15)));

        assert!(none_free(&b));
        assert_eq!(*b.get_status(), BookingStatus::Approved);
    }

    #[test]
    fn cancelling_an_approved_booking_releases_rooms() {
        let mut b = approved_booking();

        b.cancel(event_date().and_time(t(7, 0))).unwrap();

        assert!(all_free(&b));
    }

    // ---------- check-in ----------

    #[test]
    fn check_in_within_15_min_succeeds() {
        let mut b = approved_booking();

        b.check_in(event_date().and_time(t(10, 15))).unwrap();

        assert!(b.is_checked_in());
    }

    #[test]
    fn check_in_after_15_min_is_rejected() {
        let mut b = approved_booking();

        let result = b.check_in(event_date().and_time(t(10, 16)));

        assert!(matches!(result, Err(BookingError::CheckInTooLate)));
        assert!(!b.is_checked_in());
    }

    #[test]
    fn check_in_before_event_start_is_rejected() {
        let mut b = approved_booking();

        let result = b.check_in(event_date().and_time(t(9, 59)));

        assert!(matches!(result, Err(BookingError::CheckInTooEarly)));
    }

    #[test]
    fn check_in_on_non_approved_booking_is_rejected() {
        let mut b = try_rooms(vec![amphi()], 10).unwrap();

        let result = b.check_in(event_date().and_time(t(10, 0)));

        assert!(matches!(result, Err(BookingError::CheckInNotApproved(BookingStatus::Requested))));
    }

    #[test]
    fn check_in_twice_is_rejected() {
        let mut b = approved_booking();
        b.check_in(event_date().and_time(t(10, 0))).unwrap();

        let result = b.check_in(event_date().and_time(t(10, 5)));

        assert!(matches!(result, Err(BookingError::AlreadyCheckedIn)));
    }

    // ---------- refus ----------

    #[test]
    fn decline_sets_status_to_declined() {
        let mut b = booking_with_status(BookingStatus::Requested);

        b.decline();

        assert_eq!(*b.get_status(), BookingStatus::Declined);
        assert!(b.get_rooms().iter().all(|r| r.is_free()));
    }

    // ---------- annulation : statuts autorisés ----------

    #[test]
    fn cancelling_a_declined_booking_is_rejected() {
        let mut b = booking_with_status(BookingStatus::Declined);

        let result = b.cancel(event_date().and_time(t(7, 0)));

        assert!(matches!(result, Err(BookingError::CannotCancel(BookingStatus::Declined))));
    }

    #[test]
    fn cancelling_twice_is_rejected() {
        let mut b = booking_with_status(BookingStatus::Approved);
        b.cancel(event_date().and_time(t(7, 0))).unwrap();

        let result = b.cancel(event_date().and_time(t(7, 30)));

        assert!(matches!(result, Err(BookingError::CannotCancel(BookingStatus::Cancelled))));
    }

    // ---------- rappel 1h avant ----------

    #[test]
    fn reminder_is_due_1h_before_start() {
        let b = approved_booking(); // 10h–12h

        assert!(!b.reminder_due(event_date().and_time(t(8, 59))));
        assert!(b.reminder_due(event_date().and_time(t(9, 0))));
        assert!(b.reminder_due(event_date().and_time(t(9, 59))));
        assert!(!b.reminder_due(event_date().and_time(t(10, 0))));
    }

    #[test]
    fn reminder_is_not_due_twice() {
        let mut b = approved_booking();
        b.mark_reminder_sent();

        assert!(!b.reminder_due(event_date().and_time(t(9, 30))));
    }

    #[test]
    fn reminder_is_not_due_for_pending_booking() {
        let b = booking_with_status(BookingStatus::Requested);

        assert!(!b.reminder_due(event_date().and_time(t(9, 30))));
    }
}
