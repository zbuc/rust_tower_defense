#![deny(clippy::all)]

extern crate rust_tower_defense;

use rust_tower_defense::geometry::Polygon;
use rust_tower_defense::{game, graphics};

fn main() {
    let map = match game::get_default_map() {
        Ok(map) => map,
        Err(e) => panic!("Can't open default map: {}", e),
    };

    let game = game::start_game(map);

    graphics::run();
    println!("Game bbox area: {}", game.map.dimensions.area());
}
