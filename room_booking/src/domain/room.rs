#![allow(dead_code)]

use crate::domain::errors::RoomError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    Amphi,
    Standard
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    id: String,
    name: String,
    capacity: u8,
    kind: RoomType,
    is_free: bool 
}

impl Room {
    pub fn new(id: String, name: String, capacity: u8, kind: RoomType, is_free: bool) -> Result<Self, RoomError> {
        if capacity == 0 {
            return Err(RoomError::ZeroCapactity)
        }
        Ok(Self {
            id, name, capacity, kind, is_free
        })
    }

    pub fn can_host(&self, attendees: u8) -> bool {
        attendees <= self.capacity
    }

    pub fn requires_approval(&self) -> bool {
        self.kind == RoomType::Amphi
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_kind(&self) -> RoomType {
        return self.kind;
    }

    pub fn get_capacity(&self) -> u8 {
        self.capacity
    }

    pub fn is_free(&self) -> bool {
        self.is_free
    }

    pub fn occupy(&mut self) {
        self.is_free = false;
    }

    pub fn release(&mut self) {
        self.is_free = true;
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{errors::RoomError, room::{Room, RoomType}};

    #[test]
    fn new_room_with_zero_capacity_is_rejected() {
        let result = Room::new("R1".into(), "Salle A".into(), 0, RoomType::Standard, true);
        assert!(matches!(result, Err(RoomError::ZeroCapactity)))
    }
}