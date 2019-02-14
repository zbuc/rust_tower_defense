pub mod entities;
pub mod map;

use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::io::Write;

use super::geometry::Point;
use crate::bincode::{deserialize, serialize};
use entities::GameEntity;
use map::{GameMap, DEFAULT_MAP, DEFAULT_MAP_DIMENSIONS, DEFAULT_MAP_NAME};

#[derive(Debug)]
enum GameMessage {
    Interact {
        source: GameEntity,
        target: GameEntity,
    },
    TriggerAbility {
        target: GameEntity,
    },
    Move {
        target: GameEntity,
        destination: Point,
    },
}

#[derive(Debug)]
pub struct GameState {
    entities: Vec<GameEntity>,
}

#[derive(Debug)]
pub struct ActiveGame {
    pub map: GameMap,
    pub state: GameState,
    pub started_time: u32,
}

struct Cacher<T>
    where T: Fn(u32) -> u32
{
    calculation: T,
    value: Option<u32>,
}

impl<T> Cacher<T>
    where T: Fn(u32) -> u32
{
    fn new(calculation: T) -> Cacher<T> {
        Cacher {
            calculation,
            value: None,
        }
    }

    fn value(&mut self, arg: u32) -> u32 {
        match self.value {
            Some(v) => v,
            None => {
                let v = (self.calculation)(arg);
                self.value = Some(v);
                v
            },
        }
    }
}

pub fn recreate_default_map() -> Result<GameMap, Box<dyn Error>> {
    println!("Recreating default map");

    let mut file = File::create(DEFAULT_MAP)?;

    let map_struct = GameMap {
        dimensions: DEFAULT_MAP_DIMENSIONS,
        name: DEFAULT_MAP_NAME.to_string(),
    };

    let encoded: Vec<u8> = serialize(&map_struct).unwrap();
    file.write_all(&encoded)?;

    Ok(map_struct)
}

pub fn get_default_map() -> Result<GameMap, Box<dyn Error>> {
    // I will probably want to use some human-readable JSON config for top-level
    // map configurations.
    let mut map_file = match File::open(DEFAULT_MAP) {
        Ok(f) => f,
        Err(_error) => return recreate_default_map(),
    };
    let mut map_data = Vec::<u8>::new();
    map_file.read_to_end(&mut map_data)?;

    match deserialize(&map_data) {
        Ok(map_struct) => Ok(map_struct),
        Err(_error) => recreate_default_map(),
    }
}

pub fn start_game(map: GameMap) -> ActiveGame {
    ActiveGame {
        map,
        state: GameState {
            entities: Vec::new(),
        },
        started_time: 0,
    }
}
