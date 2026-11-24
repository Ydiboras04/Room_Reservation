//! Couche infrastructure : implémentations concrètes des ports de l'application.

pub mod http;
pub mod in_memory;
pub mod scheduler;
pub mod state;

mod clock;
mod console_notifier;

pub use clock::SystemClock;
pub use console_notifier::ConsoleNotifier;
