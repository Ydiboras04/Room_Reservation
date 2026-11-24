use crate::{application::ports::RepositoryError, domain::user::User};

/// Utilisateurs de l'application (demandeurs et responsables).
pub trait UserRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError>;
    fn save(&mut self, user: User) -> Result<(), RepositoryError>;
}
