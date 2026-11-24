use std::sync::RwLock;

use super::{read, upsert, write};
use crate::{
    application::ports::{RepositoryError, UserRepository},
    domain::user::User,
};

/// Utilisateurs en mémoire.
#[derive(Default)]
pub struct InMemoryUserRepository {
    users: RwLock<Vec<User>>,
}

impl InMemoryUserRepository {
    pub fn with(users: Vec<User>) -> Self {
        Self { users: RwLock::new(users) }
    }
}

impl UserRepository for InMemoryUserRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        Ok(read(&self.users)?.iter().find(|u| u.get_id() == id).cloned())
    }

    async fn save(&self, user: User) -> Result<(), RepositoryError> {
        upsert(&mut *write(&self.users)?, user, |a, b| a.get_id() == b.get_id());
        Ok(())
    }
}
