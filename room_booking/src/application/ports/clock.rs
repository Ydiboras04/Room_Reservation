use chrono::NaiveDateTime;

/// Source de l'heure actuelle (remplaçable par une horloge fixe dans les tests).
/// Reste synchrone : lire l'heure ne fait aucune entrée/sortie.
pub trait Clock: Send + Sync {
    fn now(&self) -> NaiveDateTime;
}
