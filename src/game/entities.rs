use crate::geometry::{self, Location};

// Design decision: avoid embedding methods within entities --
// we will go with a very poor version of ECS pattern (entity component system)
#[derive(Debug, Copy, Clone)]
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

impl Location for GameEntity {
    fn get_center_point(&self) -> geometry::Point {
        self.location
    }
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

pub struct Zone {
    bounding_box: geometry::BoundingBox,
}

impl Zone {
    pub fn entity_inside<'a>(&self, entity: &'a GameEntity) -> bool {
        entity.location.inside(self.get_bounding_box())
    }

    pub fn get_bounding_box(&self) -> geometry::BoundingBox {
        self.bounding_box
    }
}

#[cfg(test)]
mod tests {
    use crate::game::entities::*;
    use crate::geometry::{BoundingBox, Point};

    #[test]
    fn entities_tests() {
        let player = GameEntity {
            location: Point::new(5, 5),
            entity_type: GameEntityType::Player,
        };

        let zone = Zone {
            bounding_box: BoundingBox::new(Point::new(0, 0), Point::new(10, 10)),
        };

        assert!(zone.entity_inside(&player));
    }
}
