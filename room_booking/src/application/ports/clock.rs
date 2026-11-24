use chrono::NaiveDateTime;

/// Source de l'heure actuelle (remplaçable par une horloge fixe dans les tests).
pub trait Clock {
    fn now(&self) -> NaiveDateTime;
}
