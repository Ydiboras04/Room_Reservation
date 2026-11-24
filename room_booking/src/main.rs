//! Point d'entrée : branche les adaptateurs (infrastructure) sur les use cases
//! et démarre l'API HTTP. Aucune logique métier ici.
//!
//! Variables d'environnement :
//! - `ADDR` : adresse d'écoute (défaut `127.0.0.1:3000`) ;
//! - `CORS_ORIGINS` : origines autorisées à appeler l'API depuis un navigateur, séparées par
//!   des virgules (défaut : le serveur de dev Vite, `http://localhost:5173,http://127.0.0.1:5173`).

use std::{sync::Arc, time::Duration};

use room_booking::{
    domain::{
        room::{Room, RoomType},
        user::{User, UserRole},
    },
    infrastructure::{
        ConsoleNotifier, SystemClock,
        http::{DOCS_PATH, cors_layer, parse_origins, router},
        in_memory::{InMemoryBookingRepository, InMemoryRoomRepository, InMemoryUserRepository},
        scheduler::spawn_scheduled_checks,
        state::{AppState, InMemory},
    },
};
use tokio::net::TcpListener;

/// Fréquence des contrôles périodiques (rappels, expirations, no-shows, fins d'événement).
const SCHEDULED_CHECKS_PERIOD: Duration = Duration::from_secs(60);

const DEFAULT_CORS_ORIGINS: &str = "http://localhost:5173,http://127.0.0.1:5173";

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState::<InMemory>::new(
        seed_users(),
        seed_rooms(),
        InMemoryBookingRepository::default(),
        ConsoleNotifier,
        SystemClock,
    ));

    spawn_scheduled_checks(state.clone(), SCHEDULED_CHECKS_PERIOD);

    let cors_origins = std::env::var("CORS_ORIGINS").unwrap_or_else(|_| DEFAULT_CORS_ORIGINS.to_string());
    let cors = cors_layer(parse_origins(&cors_origins).unwrap_or_else(|error| panic!("{error}")));

    let addr = std::env::var("ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|error| panic!("impossible d'écouter sur {addr} : {error}"));
    println!("API de réservation à l'écoute sur http://{addr}");
    println!("CORS autorisé pour : {cors_origins}");
    println!("Documentation : http://{addr}{DOCS_PATH}");

    axum::serve(listener, router(state).layer(cors))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            println!("\narrêt du serveur");
        })
        .await
        .expect("le serveur HTTP s'est arrêté sur une erreur");
}

/// Données de départ : les dépôts sont en mémoire, tout est perdu à l'arrêt.
fn seed_users() -> InMemoryUserRepository {
    InMemoryUserRepository::with(vec![
        User::new("A-001".into(), "Alice".into(), UserRole::Applicant),
        User::new("A-002".into(), "Bob".into(), UserRole::Applicant),
        User::new("RESP-001".into(), "Responsable".into(), UserRole::Responsible),
    ])
}

fn seed_rooms() -> InMemoryRoomRepository {
    InMemoryRoomRepository::with(vec![
        Room::new("R-101".into(), "Salle 101".into(), 30, RoomType::Standard, true).expect("salle valide"),
        Room::new("R-102".into(), "Salle 102".into(), 20, RoomType::Standard, true).expect("salle valide"),
        Room::new("R-001".into(), "Amphi A".into(), 100, RoomType::Amphi, true).expect("salle valide"),
    ])
}
