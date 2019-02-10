#[macro_use]
extern crate serde;
extern crate bincode;
extern crate serde_derive;
extern crate serde_json;

use rand::Rng;
use std::cmp::Ordering;
use std::io;

mod game;
mod geometry;

fn main() {
    let map = match game::get_default_map() {
        Ok(map) => map,
        Err(e) => panic!("Can't open default map: {}", e),
    };

    let game = game::start_game(map);

    println!("Game started: {:#?}", game);

    println!("Game bbox area: {}", game.map.dimensions.area());

    println!("Guess the number!");

    let secret_number: u8 = rand::thread_rng().gen_range(1, 101);

    loop {
        println!("The secret number is: {}", secret_number);

        println!("Please input your guess.");

        let mut guess = String::new();

        io::stdin()
            .read_line(&mut guess)
            .expect("Failed to read line");

        let guess: u8 = match guess.trim().parse() {
            Ok(num) => num,
            Err(_) => continue,
        };

        println!("You guessed: {}", guess);

        match guess.cmp(&secret_number) {
            Ordering::Less => println!("Too small!"),
            Ordering::Greater => println!("Too big!"),
            Ordering::Equal => {
                println!("You win!");
                break;
            }
        }
    }
}
