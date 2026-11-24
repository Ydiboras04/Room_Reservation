use std::sync::RwLock;

use super::{read, upsert, write};
use crate::{
    application::ports::{RepositoryError, RoomRepository},
    domain::room::Room,
};

/// Catalogue de salles en mémoire.
#[derive(Default)]
pub struct InMemoryRoomRepository {
    rooms: RwLock<Vec<Room>>,
}

impl InMemoryRoomRepository {
    pub fn with(rooms: Vec<Room>) -> Self {
        Self { rooms: RwLock::new(rooms) }
    }
}

impl RoomRepository for InMemoryRoomRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Room>, RepositoryError> {
        Ok(read(&self.rooms)?.iter().find(|r| r.get_id() == id).cloned())
    }

    async fn find_all(&self) -> Result<Vec<Room>, RepositoryError> {
        Ok(read(&self.rooms)?.clone())
    }

    async fn save(&self, room: Room) -> Result<(), RepositoryError> {
        upsert(&mut *write(&self.rooms)?, room, |a, b| a.get_id() == b.get_id());
        Ok(())
    }
}
