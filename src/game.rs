use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::io::Write;

use crate::geometry;

use crate::bincode::{deserialize, serialize};

pub mod entities;
pub mod map;

const DEFAULT_MAP: &str = "data/map.bin";

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

pub fn recreate_default_map() -> Result<map::GameMap, Box<dyn Error>> {
    println!("Recreating default map");

    let mut file = File::create(DEFAULT_MAP)?;

    let map_struct = map::GameMap {
        dimensions: geometry::BoundingBox(geometry::Point(0, 0), geometry::Point(100, 100)),
        name: "Default Map".to_string(),
    };

    let encoded: Vec<u8> = serialize(&map_struct).unwrap();
    file.write(&encoded)?;

    Ok(map_struct)
}

pub fn get_default_map() -> Result<map::GameMap, Box<dyn Error>> {
    // I will probably want to use some human-readable JSON config for top-level
    // map configurations.
    let mut map_file = match File::open(DEFAULT_MAP) {
        Ok(f) => f,
        Err(error) => return recreate_default_map(),
    };
    let mut map_data = Vec::<u8>::new();
    map_file.read_to_end(&mut map_data)?;

    match deserialize(&map_data) {
        Ok(map_struct) => Ok(map_struct),
        Err(error) => recreate_default_map(),
    }
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
