//! État partagé de l'application : les adaptateurs branchés sur les ports,
//! utilisés à la fois par l'API HTTP et par les contrôles périodiques.

use tokio::sync::{Mutex, MutexGuard};

use crate::{
    application::ports::{BookingRepository, Clock, Notifier, RoomRepository, UserRepository},
    infrastructure::{
        ConsoleNotifier, SystemClock,
        in_memory::{InMemoryBookingRepository, InMemoryRoomRepository, InMemoryUserRepository},
    },
};

/// Le choix des adaptateurs concrets, regroupé en un seul paramètre de type.
///
/// Les handlers HTTP sont génériques sur `A: Adapters` au lieu de l'être sur cinq types :
/// la production utilise [`InMemory`], les tests une horloge figée et un faux notifier,
/// et passer à PostgreSQL ne demandera qu'une nouvelle implémentation de ce trait.
pub trait Adapters: Send + Sync + 'static {
    type Users: UserRepository + 'static;
    type Rooms: RoomRepository + 'static;
    type Bookings: BookingRepository + 'static;
    type Notifier: Notifier + 'static;
    type Clock: Clock + 'static;
}

/// Adaptateurs actuels : dépôts en mémoire, notifications dans la console, heure système.
pub struct InMemory;

impl Adapters for InMemory {
    type Users = InMemoryUserRepository;
    type Rooms = InMemoryRoomRepository;
    type Bookings = InMemoryBookingRepository;
    type Notifier = ConsoleNotifier;
    type Clock = SystemClock;
}

pub struct AppState<A: Adapters> {
    pub users: A::Users,
    pub rooms: A::Rooms,
    pub bookings: A::Bookings,
    pub notifier: A::Notifier,
    pub clock: A::Clock,
    writes: Mutex<()>,
}

impl<A: Adapters> AppState<A> {
    pub fn new(users: A::Users, rooms: A::Rooms, bookings: A::Bookings, notifier: A::Notifier, clock: A::Clock) -> Self {
        Self { users, rooms, bookings, notifier, clock, writes: Mutex::new(()) }
    }

    /// Verrou à prendre autour de chaque use case qui modifie des réservations.
    ///
    /// Les use cases lisent puis écrivent (ex. `submit_booking` : contrôle des conflits R3
    /// puis enregistrement). Sans ce verrou, deux requêtes simultanées sur le même créneau
    /// pourraient passer le contrôle toutes les deux et être enregistrées toutes les deux.
    /// Avec les dépôts en mémoire, sérialiser les écritures est la solution la plus simple ;
    /// avec une vraie base, ce rôle reviendra aux transactions et aux contraintes SQL.
    pub async fn lock_writes(&self) -> MutexGuard<'_, ()> {
        self.writes.lock().await
    }
}
