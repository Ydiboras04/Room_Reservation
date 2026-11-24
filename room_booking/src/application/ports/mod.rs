#![allow(dead_code)]

//! Ports de la couche application : les traits que l'infrastructure implémente.

mod booking_repository;
mod clock;
mod notifier;
mod repository_error;
mod room_repository;
mod user_repository;

pub use booking_repository::BookingRepository;
pub use clock::Clock;
pub use notifier::Notifier;
pub use repository_error::RepositoryError;
pub use room_repository::RoomRepository;
pub use user_repository::UserRepository;
