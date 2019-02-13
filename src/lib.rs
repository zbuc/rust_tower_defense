pub mod game;
pub mod geometry;

extern crate bincode;
extern crate serde;
extern crate serde_derive;
extern crate serde_json;

use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::io::Write;

use crate::bincode::{deserialize, serialize};
use game::entities::GameEntity;
use game::map::{GameMap, DEFAULT_MAP, DEFAULT_MAP_DIMENSIONS, DEFAULT_MAP_NAME};
