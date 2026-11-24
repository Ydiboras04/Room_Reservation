use crate::{application::ports::{RepositoryError, UserRepository}, domain::user::User};

/// Utilisateurs en mémoire.
#[derive(Default)]
pub struct InMemoryUserRepository {
    users: Vec<User>,
}

impl InMemoryUserRepository {
    pub fn with(users: Vec<User>) -> Self {
        Self { users }
    }
}

impl UserRepository for InMemoryUserRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        Ok(self.users.iter().find(|u| u.get_id() == id).cloned())
    }

    fn save(&mut self, user: User) -> Result<(), RepositoryError> {
        match self.users.iter_mut().find(|u| u.get_id() == user.get_id()) {
            Some(existing) => *existing = user,
            None => self.users.push(user),
        }
        Ok(())
    }
}
