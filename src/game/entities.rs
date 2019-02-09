use crate::geometry;

#[derive(Debug)]
pub enum GameEntityType {
    Player,
    Enemy,
    Structure,
    Zone,
    Projectile,
}

#[derive(Debug)]
pub struct GameEntity {
    pub location: geometry::Point,
    pub entity_type: GameEntityType,
}

impl GameEntity {
    fn can_take_damage(&self) -> bool {
        match self.entity_type {
            GameEntityType::Player => true,
            GameEntityType::Enemy => true,
            GameEntityType::Structure => true,
            GameEntityType::Zone => false,
            GameEntityType::Projectile => false,
        }
    }
}