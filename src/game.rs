use std::error::Error;
use std::fs;

use crate::geometry;

pub mod entities;
pub mod map;

const DEFAULT_MAP: &str = "data/map.json";

#[derive(Debug)]
enum GameMessage {
    Interact {
        source: entities::GameEntity,
        target: entities::GameEntity,
    },
    TriggerAbility {
        target: entities::GameEntity,
    },
    Move {
        target: entities::GameEntity,
        destination: geometry::Point,
    },
}

#[derive(Debug)]
pub struct GameState {
    entities: Vec<entities::GameEntity>,
}

#[derive(Debug)]
pub struct ActiveGame {
    pub map: map::GameMap,
    pub state: GameState,
    pub started_time: u32,
}

pub fn get_default_map() -> Result<map::GameMap, Box<dyn Error>> {
    // JSON is probably woefully inadequate for representing this map,
    // I will probably want to use some sort of more packed binary format,
    // maybe with an accompanying human-readable JSON config for top-level
    // map configurations.
    let map_data = fs::read_to_string(DEFAULT_MAP)?;

    let map_struct = serde_json::from_str(&map_data)?;
    Ok(map_struct)
}

pub fn start_game(map: map::GameMap) -> ActiveGame {
    ActiveGame {
        map: map,
        state: GameState {
            entities: Vec::new(),
        },
        started_time: 0,
    }
}
