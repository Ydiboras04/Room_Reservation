use crate::{application::ports::{RepositoryError, RoomRepository}, domain::room::Room};

/// Catalogue de salles en mémoire.
#[derive(Default)]
pub struct InMemoryRoomRepository {
    rooms: Vec<Room>,
}

impl InMemoryRoomRepository {
    pub fn with(rooms: Vec<Room>) -> Self {
        Self { rooms }
    }
}

impl RoomRepository for InMemoryRoomRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Room>, RepositoryError> {
        Ok(self.rooms.iter().find(|r| r.get_id() == id).cloned())
    }

    fn find_all(&self) -> Result<Vec<Room>, RepositoryError> {
        Ok(self.rooms.clone())
    }

    fn save(&mut self, room: Room) -> Result<(), RepositoryError> {
        match self.rooms.iter_mut().find(|r| r.get_id() == room.get_id()) {
            Some(existing) => *existing = room,
            None => self.rooms.push(room),
        }
        Ok(())
    }
}
