use crate::{application::ports::RepositoryError, domain::user::User};

/// Utilisateurs de l'application (demandeurs et responsables).
/// (Asynchrone et partagé : voir `BookingRepository`.)
pub trait UserRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> impl Future<Output = Result<Option<User>, RepositoryError>> + Send;
    fn save(&self, user: User) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
