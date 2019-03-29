#![deny(clippy::all)]

extern crate rust_tower_defense;

use rust_tower_defense::geometry::Polygon;
use rust_tower_defense::{game, graphics};

fn main() {
    game::run();
}
