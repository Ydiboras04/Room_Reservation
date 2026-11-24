#![allow(dead_code)]

use chrono::{NaiveDate, NaiveTime};

use crate::{
    application::{
        errors::ApplicationError,
        ports::{BookingRepository, Clock, Notifier, RoomRepository, UserRepository},
        use_cases::common::load_user,
    },
    domain::{
        errors::SystemError,
        room::Room,
        system::{BookingRequest, SubmissionOutcome, System},
    },
};

/// Ce que le demandeur saisit (« Choisir la salle et le créneau »).
/// - le demandeur et les salles sont désignés par leur id : capacité et type viennent du catalogue,
///   jamais de l'appelant (sinon R2 et R5 pourraient être contournées) ;
/// - l'heure de la demande vient de l'horloge.
#[derive(Debug, Clone)]
pub struct SubmitBookingCommand {
    pub id: String,
    pub applicant_id: String,
    pub room_ids: Vec<String>,
    pub participants: u8,
    pub event_date: NaiveDate,
    pub event_start: NaiveTime,
    pub event_end: NaiveTime,
}

/// Use case « Soumettre la demande » :
/// 1. charge le demandeur et les salles demandées ;
/// 2. horodate la demande ;
/// 3. charge les réservations du jour ;
/// 4. laisse le domaine appliquer R1–R5 ;
/// 5. enregistre la réservation acceptée ;
/// 6. notifie le résultat.
pub async fn submit_booking(
    command: SubmitBookingCommand,
    users: &impl UserRepository,
    rooms: &impl RoomRepository,
    repo: &impl BookingRepository,
    clock: &impl Clock,
    notifier: &impl Notifier,
) -> Result<SubmissionOutcome, ApplicationError> {
    let booking_id = command.id.clone();

    let loaded = async {
        let applicant = load_user(users, &command.applicant_id).await?;
        let rooms = load_rooms(rooms, &command.room_ids).await?;
        Ok::<_, ApplicationError>((applicant, rooms))
    }
    .await;
    let (applicant, rooms) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            // Une panne du stockage n'est pas un refus de la demande : rien à notifier.
            if !matches!(error, ApplicationError::Repository(_)) {
                notifier.notify_rejected(&booking_id, &error.to_string()).await;
            }
            return Err(error);
        }
    };

    let request = BookingRequest {
        id: command.id,
        applicant,
        rooms,
        participants: command.participants,
        event_date: command.event_date,
        event_start: command.event_start,
        event_end: command.event_end,
        requested_at: clock.now(),
    };

    let existing = repo.find_by_date(request.event_date).await?;

    match System::submit_request(request, &existing) {
        Ok(outcome) => {
            match &outcome {
                SubmissionOutcome::Confirmed(booking) => {
                    repo.save(booking.clone()).await?;
                    notifier.notify_confirmed(booking).await;
                }
                SubmissionOutcome::PendingApproval(booking) => {
                    repo.save(booking.clone()).await?;
                    notifier.notify_pending_approval(booking).await;
                }
            }
            Ok(outcome)
        }
        Err(error) => {
            match &error {
                SystemError::InvalidRequest(reason) => {
                    notifier.notify_rejected(&booking_id, &reason.to_string()).await;
                }
                SystemError::SlotUnavailable { alternatives } => {
                    notifier.notify_slot_unavailable(&booking_id, alternatives).await;
                }
            }
            Err(error.into())
        }
    }
}

/// Charge chaque salle depuis le catalogue ; la première inconnue arrête tout.
/// Liste vide et doublons sont laissés au domaine (NoRoomSelected, DuplicateRoom).
async fn load_rooms(rooms: &impl RoomRepository, room_ids: &[String]) -> Result<Vec<Room>, ApplicationError> {
    let mut loaded = Vec::with_capacity(room_ids.len());
    for id in room_ids {
        let room = rooms.find_by_id(id).await?.ok_or_else(|| ApplicationError::RoomNotFound(id.clone()))?;
        loaded.push(room);
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;

    use crate::{
        application::{
            errors::ApplicationError,
            fakes::{
                FailingRepository, FakeNotifier, FixedClock, InMemoryBookingRepository, InMemoryRoomRepository,
                Notification, users,
            },
            ports::BookingRepository,
            use_cases::submit_booking::{SubmitBookingCommand, submit_booking},
        },
        domain::{
            booking::{
                BookingStatus,
                tests::{amphi, booking, event_date, requested_at, salle_101, t},
            },
            errors::{BookingError, SystemError},
            system::{SubmissionOutcome, TimeSlot},
        },
    };

    /// Catalogue : R-101 (standard, 30 places) et R-001 (amphi, 100 places).
    fn catalog() -> InMemoryRoomRepository {
        InMemoryRoomRepository::with(vec![salle_101(), amphi()])
    }

    fn command(room_ids: &[&str], participants: u8, start: NaiveTime, end: NaiveTime) -> SubmitBookingCommand {
        SubmitBookingCommand {
            id: "B-NEW".into(),
            applicant_id: "A-001".into(),
            room_ids: room_ids.iter().map(|id| id.to_string()).collect(),
            participants,
            event_date: event_date(),
            event_start: start,
            event_end: end,
        }
    }

    #[tokio::test]
    async fn standard_room_booking_is_confirmed_saved_and_notified() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101"], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Ok(SubmissionOutcome::Confirmed(_))));
        let saved = repo.find_by_id("B-NEW").await.unwrap().expect("la réservation doit être enregistrée");
        assert_eq!(*saved.get_status(), BookingStatus::Approved);
        assert_eq!(notifier.sent(), vec![Notification::Confirmed("B-NEW".into())]);
    }

    #[tokio::test]
    async fn amphi_booking_is_saved_as_pending_and_responsible_notified() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-001"], 50, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Ok(SubmissionOutcome::PendingApproval(_))));
        assert_eq!(*repo.find_by_id("B-NEW").await.unwrap().unwrap().get_status(), BookingStatus::Requested);
        assert_eq!(notifier.sent(), vec![Notification::PendingApproval("B-NEW".into())]);
    }

    #[tokio::test]
    async fn request_time_comes_from_the_clock() {
        // Horloge réglée la veille à 15h → l'événement de 10h est à 19h : R4 refuse.
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(event_date().pred_opt().unwrap().and_time(t(15, 0)));
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101"], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::System(SystemError::InvalidRequest(BookingError::RequestTooLateOrTooEarly)))));
    }

    #[tokio::test]
    async fn invalid_booking_is_not_saved_and_rejection_is_notified() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        // 40 personnes en salle 101 (30 places) → R2
        let result = submit_booking(command(&["R-101"], 40, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::System(SystemError::InvalidRequest(BookingError::ParticipantsSmallerThanRoomCapacity)))
        ));
        assert!(repo.find_by_id("B-NEW").await.unwrap().is_none());
        assert!(matches!(notifier.sent().as_slice(), [Notification::Rejected(id, _)] if id == "B-NEW"));
    }

    #[tokio::test]
    async fn conflicting_booking_is_not_saved_and_alternatives_are_notified() {
        let repo = InMemoryBookingRepository::default();
        repo.save(booking("B-001", salle_101(), BookingStatus::Approved, t(8, 0), t(10, 0))).await.unwrap();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101"], 20, t(9, 30), t(10, 30)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::System(SystemError::SlotUnavailable { .. }))));
        assert!(repo.find_by_id("B-NEW").await.unwrap().is_none());
        match notifier.sent().as_slice() {
            [Notification::SlotUnavailable(id, alternatives)] => {
                assert_eq!(id, "B-NEW");
                assert!(alternatives.contains(&TimeSlot { start: t(10, 0), end: t(11, 0) }));
            }
            other => panic!("notification inattendue : {other:?}"),
        }
    }

    #[tokio::test]
    async fn second_booking_on_same_slot_conflicts_with_the_first_one() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let first = submit_booking(command(&["R-101"], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;
        let mut second_cmd = command(&["R-101"], 20, t(10, 0), t(11, 0));
        second_cmd.id = "B-SECOND".into();
        let second = submit_booking(second_cmd, &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(first.is_ok());
        assert!(matches!(second, Err(ApplicationError::System(SystemError::SlotUnavailable { .. }))));
        assert!(repo.find_by_id("B-SECOND").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn unknown_room_is_rejected_notified_and_not_saved() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101", "R-404"], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::RoomNotFound(id)) if id == "R-404"));
        assert!(repo.find_by_id("B-NEW").await.unwrap().is_none());
        assert!(matches!(notifier.sent().as_slice(), [Notification::Rejected(id, _)] if id == "B-NEW"));
    }

    #[tokio::test]
    async fn room_type_comes_from_the_catalog() {
        // L'appelant ne donne qu'un id : impossible de faire passer l'amphi pour une salle standard.
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-001", "R-101"], 50, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Ok(SubmissionOutcome::PendingApproval(_))));
    }

    #[tokio::test]
    async fn same_room_id_twice_is_rejected_by_the_domain() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101", "R-101"], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::System(SystemError::InvalidRequest(BookingError::DuplicateRoom(id)))) if id == "R-101"
        ));
    }

    #[tokio::test]
    async fn empty_room_list_is_rejected_by_the_domain() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&[], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(
            result,
            Err(ApplicationError::System(SystemError::InvalidRequest(BookingError::NoRoomSelected)))
        ));
    }

    #[tokio::test]
    async fn unknown_applicant_is_rejected_notified_and_not_saved() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();
        let mut cmd = command(&["R-101"], 20, t(10, 0), t(11, 0));
        cmd.applicant_id = "U-404".into();

        let result = submit_booking(cmd, &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::UserNotFound(id)) if id == "U-404"));
        assert!(repo.find_by_id("B-NEW").await.unwrap().is_none());
        assert!(matches!(notifier.sent().as_slice(), [Notification::Rejected(id, _)] if id == "B-NEW"));
    }

    #[tokio::test]
    async fn a_responsible_can_also_book() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();
        let mut cmd = command(&["R-101"], 20, t(10, 0), t(11, 0));
        cmd.applicant_id = "RESP-001".into();

        let result = submit_booking(cmd, &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(result.is_ok());
        assert_eq!(repo.find_by_id("B-NEW").await.unwrap().unwrap().get_applicant().get_id(), "RESP-001");
    }

    #[tokio::test]
    async fn storage_failure_on_save_is_returned_and_nothing_is_notified() {
        let repo = FailingRepository;
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101"], 20, t(10, 0), t(11, 0)), &users(), &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::Repository(_))));
        assert!(notifier.sent().is_empty(), "la demande ne doit pas être annoncée confirmée");
    }

    #[tokio::test]
    async fn storage_failure_when_loading_applicant_is_not_notified_as_a_rejection() {
        let repo = InMemoryBookingRepository::default();
        let clock = FixedClock(requested_at());
        let notifier = FakeNotifier::default();

        let result = submit_booking(command(&["R-101"], 20, t(10, 0), t(11, 0)), &FailingRepository, &catalog(), &repo, &clock, &notifier).await;

        assert!(matches!(result, Err(ApplicationError::Repository(_))));
        assert!(notifier.sent().is_empty());
        assert!(repo.find_by_id("B-NEW").await.unwrap().is_none());
    }
}
