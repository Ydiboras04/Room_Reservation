//! Dépôts en mémoire (un `Vec` chacun) : pour les tests et pour faire tourner
//! l'application avant d'avoir une vraie base de données. Les données sont perdues à l'arrêt.
//!
//! Chaque `Vec` est protégé par un `RwLock` : les ports prennent `&self` et le dépôt est
//! partagé entre requêtes concurrentes. Le verrou de `std` suffit car il n'est jamais tenu
//! pendant un `.await` (chaque méthode le prend et le relâche sans rien attendre).

use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::application::ports::RepositoryError;

mod booking_repository;
mod room_repository;
mod user_repository;

pub use booking_repository::InMemoryBookingRepository;
pub use room_repository::InMemoryRoomRepository;
pub use user_repository::InMemoryUserRepository;

/// Un verrou « empoisonné » signifie qu'un thread a paniqué en écrivant :
/// les données peuvent être incohérentes, on le signale comme une panne du stockage.
fn poisoned<T>(_: PoisonError<T>) -> RepositoryError {
    RepositoryError::storage("dépôt en mémoire corrompu (un thread a paniqué en écrivant)")
}

fn read<T>(lock: &RwLock<T>) -> Result<RwLockReadGuard<'_, T>, RepositoryError> {
    lock.read().map_err(poisoned)
}

fn write<T>(lock: &RwLock<T>) -> Result<RwLockWriteGuard<'_, T>, RepositoryError> {
    lock.write().map_err(poisoned)
}

/// Insère ou remplace l'élément de même id.
fn upsert<T>(items: &mut Vec<T>, item: T, same_id: impl Fn(&T, &T) -> bool) {
    match items.iter_mut().find(|existing| same_id(existing, &item)) {
        Some(existing) => *existing = item,
        None => items.push(item),
    }
}
