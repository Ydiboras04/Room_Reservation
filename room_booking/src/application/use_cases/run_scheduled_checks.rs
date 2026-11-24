#![allow(dead_code)]

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, Clock, Notifier},
    },
    domain::{
        booking::{Booking, BookingStatus},
        system::System,
    },
};

/// Ce qui a changé pendant un passage des contrôles (utile pour les logs et les tests).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScheduledChecksReport {
    /// « 1 h avant le début » → « Envoyer un rappel ».
    pub reminders_sent: Vec<String>,
    /// « 24 h sans réponse » → Expirée.
    pub expired: Vec<String>,
    /// « Début + 15 min sans check-in » → Libérer la salle → Non présenté.
    pub no_shows: Vec<String>,
    /// Événement terminé → salles libérées.
    pub completed: Vec<String>,
}

/// Use case « contrôles périodiques » du couloir Système, à lancer régulièrement
/// (par exemple toutes les minutes) par l'infrastructure :
/// 1. charge les réservations actives ;
/// 2. le domaine applique expiration, no-show, fin d'événement, et indique les rappels dus ;
/// 3. envoie les rappels, puis les marque comme envoyés ;
/// 4. enregistre les réservations et renvoie un rapport.
///
/// Si un enregistrement échoue, on s'arrête là : les réservations déjà enregistrées le restent,
/// les autres seront retraitées au prochain passage (leurs changements sont recalculés à partir
/// de l'heure). Seul effet de bord possible : un rappel envoyé mais pas enregistré repartira.
pub async fn run_scheduled_checks(
    repo: &impl BookingRepository,
    clock: &impl Clock,
    notifier: &impl Notifier,
) -> Result<ScheduledChecksReport, ApplicationError> {
    let now = clock.now();
    let mut bookings = repo.find_active().await?;
    let before: Vec<Snapshot> = bookings.iter().map(Snapshot::of).collect();

    let reminder_ids = System::run_scheduled_checks(&mut bookings, now);

    let mut report = ScheduledChecksReport::default();

    for booking in bookings.iter_mut() {
        if reminder_ids.iter().any(|id| id == booking.get_id()) {
            notifier.send_reminder(booking).await;
            // Marqué après l'envoi, pour ne pas prétendre qu'un rappel est parti s'il a échoué.
            booking.mark_reminder_sent();
            report.reminders_sent.push(booking.get_id().to_string());
        }
    }

    for (booking, was) in bookings.into_iter().zip(before) {
        let id = booking.get_id().to_string();
        let after = Snapshot::of(&booking);

        if !was.expired && after.expired {
            report.expired.push(id.clone());
        }
        if was.status != BookingStatus::NoShow && after.status == BookingStatus::NoShow {
            report.no_shows.push(id.clone());
        } else if after.status == BookingStatus::Approved && !was.rooms_free && after.rooms_free {
            report.completed.push(id.clone());
        }

        repo.save(booking).await?;
    }

    Ok(report)
}

/// État d'une réservation avant/après les contrôles, pour construire le rapport.
struct Snapshot {
    status: BookingStatus,
    expired: bool,
    rooms_free: bool,
}

impl Snapshot {
    fn of(booking: &Booking) -> Self {
        Self {
            status: *booking.get_status(),
            expired: booking.is_expired(),
            rooms_free: booking.get_rooms().iter().all(|r| r.is_free()),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{FailingRepository, FakeNotifier, FixedClock, InMemoryBookingRepository, Notification},
            ports::BookingRepository,
            use_cases::run_scheduled_checks::{ScheduledChecksReport, run_scheduled_checks},
        },
        domain::booking::{
            Booking, BookingStatus,
            tests::{amphi, booking, event_date, requested_at, salle_101, t},
        },
    };

    /// Réservation approuvée (salles réellement bloquées), de 10h à 12h.
    fn approved(id: &str) -> Booking {
        let mut b = booking(id, salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0));
        b.approve();
        b
    }

    #[tokio::test]
    async fn reminder_is_sent_once_1h_before() {
        let repo = InMemoryBookingRepository::default();
        repo.save(approved("B-001")).await.unwrap();
        let notifier = FakeNotifier::default();

        let first = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(9, 0))), &notifier).await.unwrap();
        let second = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(9, 1))), &notifier).await.unwrap();

        assert_eq!(first.reminders_sent, vec!["B-001".to_string()]);
        assert!(second.reminders_sent.is_empty());
        assert_eq!(notifier.sent(), vec![Notification::Reminder("B-001".into())]);
        assert!(repo.find_by_id("B-001").await.unwrap().unwrap().is_reminder_sent());
    }

    #[tokio::test]
    async fn no_reminder_before_1h() {
        let repo = InMemoryBookingRepository::default();
        repo.save(approved("B-001")).await.unwrap();
        let notifier = FakeNotifier::default();

        let report = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(8, 59))), &notifier).await.unwrap();

        assert_eq!(report, ScheduledChecksReport::default());
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn pending_request_expires_after_24h_and_is_saved() {
        let repo = InMemoryBookingRepository::default();
        repo.save(booking("B-001", amphi(), BookingStatus::Requested, t(10, 0), t(12, 0))).await.unwrap();
        let notifier = FakeNotifier::default();

        let report = run_scheduled_checks(&repo, &FixedClock(requested_at() + TimeDelta::hours(24)), &notifier).await.unwrap();

        assert_eq!(report.expired, vec!["B-001".to_string()]);
        assert!(repo.find_by_id("B-001").await.unwrap().unwrap().is_expired());
        // une demande expirée n'est plus active : plus surveillée ensuite
        assert!(repo.find_active().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_check_in_after_15_min_is_a_no_show() {
        let repo = InMemoryBookingRepository::default();
        repo.save(approved("B-001")).await.unwrap();
        let notifier = FakeNotifier::default();

        let report = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(10, 16))), &notifier).await.unwrap();

        assert_eq!(report.no_shows, vec!["B-001".to_string()]);
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert_eq!(*saved.get_status(), BookingStatus::NoShow);
        assert!(saved.get_rooms().iter().all(|r| r.is_free()));
    }

    #[tokio::test]
    async fn checked_in_booking_is_released_when_event_ends() {
        let repo = InMemoryBookingRepository::default();
        let mut b = approved("B-001");
        b.check_in(event_date().and_time(t(10, 5))).unwrap();
        repo.save(b).await.unwrap();
        let notifier = FakeNotifier::default();

        let during = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(11, 0))), &notifier).await.unwrap();
        let after = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(12, 0))), &notifier).await.unwrap();

        assert!(during.completed.is_empty());
        assert_eq!(after.completed, vec!["B-001".to_string()]);
        let saved = repo.find_by_id("B-001").await.unwrap().unwrap();
        assert_eq!(*saved.get_status(), BookingStatus::Approved);
        assert!(saved.get_rooms().iter().all(|r| r.is_free()));
    }

    #[tokio::test]
    async fn several_bookings_are_handled_in_one_pass() {
        let repo = InMemoryBookingRepository::default();
        repo.save(approved("B-NOSHOW")).await.unwrap();                     // 10h–12h, pas de check-in
        let mut later = booking("B-LATER", amphi(), BookingStatus::Requested, t(11, 0), t(12, 0));
        later.approve();                                       // 11h–12h → rappel dû à 10h16
        repo.save(later).await.unwrap();
        let notifier = FakeNotifier::default();

        let report = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(10, 16))), &notifier).await.unwrap();

        assert_eq!(report.no_shows, vec!["B-NOSHOW".to_string()]);
        assert_eq!(report.reminders_sent, vec!["B-LATER".to_string()]);
    }

    #[tokio::test]
    async fn cancelled_or_declined_bookings_are_ignored() {
        let repo = InMemoryBookingRepository::default();
        repo.save(booking("B-001", salle_101(), BookingStatus::Cancelled, t(10, 0), t(12, 0))).await.unwrap();
        repo.save(booking("B-002", amphi(), BookingStatus::Declined, t(10, 0), t(12, 0))).await.unwrap();
        let notifier = FakeNotifier::default();

        let report = run_scheduled_checks(&repo, &FixedClock(event_date().and_time(t(9, 30))), &notifier).await.unwrap();

        assert_eq!(report, ScheduledChecksReport::default());
        assert!(notifier.sent().is_empty());
    }

    #[tokio::test]
    async fn storage_failure_is_returned_and_no_reminder_sent() {
        let notifier = FakeNotifier::default();

        let result = run_scheduled_checks(&FailingRepository, &FixedClock(event_date().and_time(t(9, 0))), &notifier).await;

        assert!(matches!(result, Err(ApplicationError::Repository(_))));
        assert!(notifier.sent().is_empty());
    }
}
