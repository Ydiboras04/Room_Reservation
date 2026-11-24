#![allow(dead_code)]

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike};

use crate::domain::applicant::Applicant;
use crate::domain::room::{Room, RoomType};
use crate::domain::errors::BookingError;

pub const EXPIRY_DELAY_HOURS: i64 = 72;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingStatus {
    Requested,
    Approved,
    Declined,
    Cancelled
}

#[derive(Debug, Clone)]
pub struct Booking {
    id: String,
    applicant: Applicant,
    room: Room,
    status: BookingStatus,
    participants: u8,
    event_date: NaiveDate,
    event_start: NaiveTime,
    event_end: NaiveTime,
    expired: bool,
    requested_at: NaiveDateTime,
}

impl Booking {
    pub fn new (id: String, applicant: Applicant, room: Room, status: BookingStatus, participants: u8, event_date: NaiveDate, event_start: NaiveTime, event_end: NaiveTime, requested_at: NaiveDateTime) -> Result<Self, BookingError> {
        Self::check_slot(event_start, event_end)?;                       // R1
        Self::check_capacity(&room, participants)?;                      // R2
        Self::check_lead_time(event_date.and_time(event_start), requested_at)?; // R4

        Ok(Self{
            id, applicant, room, status, participants, event_date, event_start, event_end, expired: false, requested_at
        })
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

    /// R2 : nombre de participants ≤ capacité de la salle.
    fn check_capacity(room: &Room, participants: u8) -> Result<(), BookingError> {
        if !room.can_host(participants) {
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

    pub fn set_status(&mut self, status: BookingStatus) {
        self.status = status;
    }

    pub fn get_status(&self) -> &BookingStatus {
        &self.status
    }

    pub fn is_expired(&self) -> bool {
        self.expired
    }

    pub fn event_start_at(&self) -> NaiveDateTime {
        self.event_date.and_time(self.event_start)
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

    pub fn auto_validate(&mut self) {
        if self.room.get_kind() == RoomType::Standard {
            self.status = BookingStatus::Approved;
        }
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
            BookingStatus::Declined | BookingStatus::Cancelled => false,
        }
    }

    /// Même salle, même jour, et les créneaux se chevauchent.
    /// 08:00–10:00 et 10:00–11:00 ne se chevauchent pas.
    pub fn overlaps(&self, other: &Booking) -> bool {
        self.id != other.id
            && self.room.get_id() == other.room.get_id()
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
    pub fn cancel(&mut self, now: NaiveDateTime) -> Result<(), BookingError> {
        if now > self.event_start_at() - TimeDelta::hours(MIN_CANCELLATION_HOURS) {
            return Err(BookingError::AnnulationTooLate);
        }
        self.status = BookingStatus::Cancelled;
        Ok(())
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

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn event_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()
    }

    pub(crate) fn requested_at() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 25).unwrap().and_hms_opt(9, 0, 0).unwrap()
    }

    fn amphi() -> Room {
        Room::new("R-001".into(), "Amphi A".into(), 100, RoomType::Amphi, true).unwrap()
    }

    fn salle_101() -> Room {
        Room::new("R-101".into(), "Salle 101".into(), 30, RoomType::Standard, true).unwrap()
    }

    /// Construit une réservation où seul le créneau change.
    fn try_slot(start: NaiveTime, end: NaiveTime) -> Result<Booking, BookingError> {
        Booking::new(
            "B-001".into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            amphi(),
            BookingStatus::Requested,
            10,
            event_date(),
            start,
            end,
            requested_at(),
        )
    }

    fn booking(id: &str, room: Room, status: BookingStatus, start: NaiveTime, end: NaiveTime) -> Booking {
        Booking::new(
            id.into(),
            Applicant::new("A-001".into(), "User1".into()).unwrap(),
            room,
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
            salle_101(),
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
            salle_101(),
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
        old.update_expiry(requested_at() + TimeDelta::hours(72)).unwrap();
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
            amphi(),
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
            amphi(),
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
            amphi(),
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

    // ---------- expiration (72h) ----------

    #[test]
    fn requested_booking_expires_after_72h() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        booking.update_expiry(requested_at() + TimeDelta::hours(72)).unwrap();

        assert!(booking.is_expired());
        assert!(matches!(booking.ensure_not_expired(), Err(BookingError::Expired)));
    }

    #[test]
    fn requested_booking_does_not_expire_before_72h() {
        let mut booking = booking_with_status(BookingStatus::Requested);

        booking
            .update_expiry(requested_at() + TimeDelta::hours(72) - TimeDelta::seconds(1))
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
}
