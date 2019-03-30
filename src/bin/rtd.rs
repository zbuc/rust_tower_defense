#![deny(clippy::all)]

#[macro_use] extern crate log;

extern crate rust_tower_defense;

use rust_tower_defense::geometry::Polygon;
use rust_tower_defense::{game, graphics};
use rust_tower_defense::logging;

fn main() {
    // instantiate logging
    logging::init().unwrap();

    let map = match game::get_default_map() {
        Ok(map) => map,
        Err(e) => panic!("Can't open default map: {}", e),
    };

    let game = game::start_game(map);

    graphics::run();
    info!("Game bbox area: {}", game.map.dimensions.area());
}
