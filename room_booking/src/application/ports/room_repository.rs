use crate::{application::ports::RepositoryError, domain::room::Room};

/// Catalogue des salles : la seule source de vérité pour leur capacité et leur type.
/// (Asynchrone et partagé : voir `BookingRepository`.)
pub trait RoomRepository: Send + Sync {
    fn find_by_id(&self, id: &str) -> impl Future<Output = Result<Option<Room>, RepositoryError>> + Send;
    fn find_all(&self) -> impl Future<Output = Result<Vec<Room>, RepositoryError>> + Send;
    fn save(&self, room: Room) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
