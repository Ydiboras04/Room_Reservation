//! Couche infrastructure : implémentations concrètes des ports de l'application.

pub mod in_memory;

mod clock;
mod console_notifier;

pub use clock::SystemClock;
pub use console_notifier::ConsoleNotifier;
