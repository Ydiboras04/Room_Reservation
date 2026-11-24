use chrono::{Local, NaiveDateTime};

use crate::application::ports::Clock;

/// Horloge réelle : heure locale de la machine.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> NaiveDateTime {
        Local::now().naive_local()
    }
}
