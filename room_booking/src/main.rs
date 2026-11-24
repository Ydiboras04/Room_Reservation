//! Point d'entrée : branche les adaptateurs (infrastructure) sur les use cases
//! de la bibliothèque `room_booking`. Aucune logique métier ici.
//!
//! Pour l'instant, déroule un scénario de démonstration avec des dépôts en mémoire.

use chrono::{Days, NaiveTime};

use room_booking::{
    application::{
        ports::Clock,
        use_cases::{
            run_scheduled_checks::run_scheduled_checks,
            submit_booking::{SubmitBookingCommand, submit_booking},
            validate_booking::validate_booking,
        },
    },
    domain::{
        room::{Room, RoomType},
        system::SubmissionOutcome,
        user::{User, UserRole},
    },
    infrastructure::{
        ConsoleNotifier, SystemClock,
        in_memory::{InMemoryBookingRepository, InMemoryRoomRepository, InMemoryUserRepository},
    },
};

fn main() {
    println!("Room Reservation Application\n");

    // ---------- branchement des adaptateurs ----------
    let clock = SystemClock;
    let notifier = ConsoleNotifier;
    let users = InMemoryUserRepository::with(vec![
        User::new("A-001".into(), "Alice".into(), UserRole::Applicant),
        User::new("RESP-001".into(), "Responsable".into(), UserRole::Responsible),
    ]);
    let rooms = InMemoryRoomRepository::with(vec![
        Room::new("R-101".into(), "Salle 101".into(), 30, RoomType::Standard, true).expect("salle valide"),
        Room::new("R-001".into(), "Amphi A".into(), 100, RoomType::Amphi, true).expect("salle valide"),
    ]);
    let mut bookings = InMemoryBookingRepository::default();

    // Événement dans 2 jours : respecte R4 (≥ 24h, ≤ 30 jours).
    let event_date = clock.now().date() + Days::new(2);
    let at = |h: u32| NaiveTime::from_hms_opt(h, 0, 0).expect("heure valide");
    let command = |id: &str, room: &str, participants: u8, start: u32, end: u32| SubmitBookingCommand {
        id: id.into(),
        applicant_id: "A-001".into(),
        room_ids: vec![room.into()],
        participants,
        event_date,
        event_start: at(start),
        event_end: at(end),
    };

    // ---------- scénario ----------
    println!("1. Alice réserve la salle 101 (standard) de 10h à 11h");
    report(submit_booking(command("B-1", "R-101", 20, 10, 11), &users, &rooms, &mut bookings, &clock, &notifier));

    println!("\n2. Alice redemande la salle 101 de 10h à 11h (conflit R3)");
    report(submit_booking(command("B-2", "R-101", 20, 10, 11), &users, &rooms, &mut bookings, &clock, &notifier));

    println!("\n3. Alice demande 40 places en salle 101 (capacité 30, R2)");
    report(submit_booking(command("B-3", "R-101", 40, 14, 15), &users, &rooms, &mut bookings, &clock, &notifier));

    println!("\n4. Alice réserve l'amphi A de 14h à 16h (validation requise, R5)");
    report(submit_booking(command("B-4", "R-001", 80, 14, 16), &users, &rooms, &mut bookings, &clock, &notifier));

    println!("\n5. Alice essaie de valider sa propre demande");
    match validate_booking("B-4", "A-001", &users, &mut bookings, &clock, &notifier) {
        Ok(_) => println!("  ✅ validée"),
        Err(error) => println!("  ❌ {error}"),
    }

    println!("\n6. Le responsable valide la demande");
    match validate_booking("B-4", "RESP-001", &users, &mut bookings, &clock, &notifier) {
        Ok(_) => println!("  ✅ validée"),
        Err(error) => println!("  ❌ {error}"),
    }

    println!("\n7. Contrôles périodiques");
    match run_scheduled_checks(&mut bookings, &clock, &notifier) {
        Ok(checks) => println!("  {checks:?}"),
        Err(error) => println!("  ❌ {error}"),
    }
}

fn report<E: std::fmt::Display>(result: Result<SubmissionOutcome, E>) {
    match result {
        Ok(SubmissionOutcome::Confirmed(b)) => println!("  ✅ {} confirmée", b.get_id()),
        Ok(SubmissionOutcome::PendingApproval(b)) => println!("  ⏳ {} en attente de validation", b.get_id()),
        Err(error) => println!("  ❌ {error}"),
    }
}
