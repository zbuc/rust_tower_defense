use crate::geometry;

pub mod map;
pub mod entities;

#[derive(Debug)]
enum GameMessage {
    Interact{source: entities::GameEntity, target: entities::GameEntity},
    TriggerAbility{target: entities::GameEntity},
    Move{target: entities::GameEntity, destination: geometry::Point},
}

#[derive(Debug)]
pub struct GameState {
//    entities: 
}

#[derive(Debug)]
pub struct ActiveGame {
    pub map: map::GameMap,
    pub state: GameState,
    pub started_time: u32,
}

pub fn get_default_map() -> map::GameMap {
    map::GameMap {
        name: String::from("Test"),
        dimensions: geometry::BoundingBox (
            geometry::Point(0,0),
            geometry::Point(100,100)
        )
    }
}

pub fn start_game(map: map::GameMap) -> ActiveGame {
    ActiveGame {
        map: map,
        state: GameState {
            
        },
        started_time: 0,
    }
}