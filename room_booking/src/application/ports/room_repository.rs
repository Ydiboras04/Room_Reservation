use crate::{application::ports::RepositoryError, domain::room::Room};

/// Catalogue des salles : la seule source de vérité pour leur capacité et leur type.
pub trait RoomRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Room>, RepositoryError>;
    fn find_all(&self) -> Result<Vec<Room>, RepositoryError>;
    fn save(&mut self, room: Room) -> Result<(), RepositoryError>;
}
