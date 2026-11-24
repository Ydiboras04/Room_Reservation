//! Dépôts en mémoire (un `Vec` chacun) : pour les tests et pour faire tourner
//! l'application avant d'avoir une vraie base de données. Les données sont perdues à l'arrêt.

mod booking_repository;
mod room_repository;
mod user_repository;

pub use booking_repository::InMemoryBookingRepository;
pub use room_repository::InMemoryRoomRepository;
pub use user_repository::InMemoryUserRepository;
