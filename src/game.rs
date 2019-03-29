pub mod entities;
pub mod map;

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::thread;

use super::geometry::Point;
use super::graphics::{
    GraphicalGame, GraphicsState, HalState, LocalState, Triangle, UserInput, WindowState,
};
use crate::bincode::{deserialize, serialize};
use crate::hal::format::{Aspects, Swizzle};
use crate::hal::window::{Backbuffer, Extent2D};
use core::mem::ManuallyDrop;
use entities::GameEntity;
use map::{GameMap, DEFAULT_MAP, DEFAULT_MAP_DIMENSIONS, DEFAULT_MAP_NAME};

#[derive(Debug)]
#[allow(dead_code)]
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
    pub entities: Vec<Rc<GameEntity>>,
}

#[derive(Debug)]
pub struct ActiveGame {
    pub map: GameMap,
    pub state: GameState,
    pub started_time: u32,
}

#[allow(dead_code)]
struct Cacher<T, U, V>
where
    T: Fn(U) -> V,
{
    calculation: T,
    values: HashMap<U, V>,
}

impl<T, U, V> Cacher<T, U, V>
where
    T: Fn(U) -> V,
    U: std::cmp::Eq + std::hash::Hash + std::cmp::PartialEq + Copy,
    V: std::cmp::Eq + Copy,
{
    #[allow(dead_code)]
    fn new(calculation: T) -> Cacher<T, U, V> {
        Cacher {
            calculation,
            values: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    fn value(&mut self, arg: U) -> V {
        match self.values.get(&arg) {
            Some(v) => *v,
            None => {
                let v = (self.calculation)(arg);
                self.values.insert(arg, v);
                v
            }
        }
    }
}

#[test]
fn call_with_different_values() {
    let mut c = Cacher::new(|a| a);

    let v1 = c.value(1);
    let v2 = c.value(2);

    assert_eq!(v1, 1);
    assert_eq!(v2, 2);
}

#[test]
fn call_with_different_types() {
    let mut c = Cacher::new(|_a| "some_str");

    let v1 = c.value(1);
    let v2 = c.value(2);

    assert_eq!(v1, v2);
    assert_eq!(v1, "some_str");

    let mut c = Cacher::new(|_a| 1);

    let v1 = c.value("foo");
    let v2 = c.value("bar");

    assert_eq!(v1, v2);
    assert_eq!(v1, 1);
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

/// Loads the default map from the environment variable DEFAULT_MAP,
/// and returns a Ok variant containing the default map struct, or an error.
///
/// # Errors
///
/// If there is any issue loading the map file from disk, an Err variant will
/// be returned.
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

/// Runs the main graphics event loop.
///
/// Exits when the window is closed.
/// Eventually we'll have a command system responsible
/// for telling the window to close.
pub fn run() {
    // if building in debug mode, vulkan backend initializes standard validation layers
    // all we need to do is enable logging
    // run the program like so to print all logs of level 'warn' and above:
    // bash: RUST_LOG=warn && cargo run --bin 02_validation_layers --features vulkan
    // powershell: $env:RUST_LOG="warn"; cargo run --bin 02_validation_layers --features vulkan
    // see: https://docs.rs/env_logger/0.5.13/env_logger/
    env_logger::init();
    info!("Starting the application!");
    let map = match get_default_map() {
        Ok(map) => map,
        Err(e) => panic!("Can't open default map: {}", e),
    };

    let game = start_game(map);
    let application = RustTowerDefenseApplication::init();
    application.run();
}

/// The application contains the window and graphics
/// drivers' states. This is the main interface to interact
/// with the application. We would probably expand this to be
/// the systems' manager at some point, or reference it from
/// the systems' manager.
pub struct RustTowerDefenseApplication {
    graphics_state: GraphicsState,
}

impl RustTowerDefenseApplication {}

impl GraphicalGame for RustTowerDefenseApplication {
    /// Initialize a new instance of the application. Will initialize the
    /// window and graphics state, and then return a new RustTowerDefenseApplication.
    fn init() -> RustTowerDefenseApplication {
        RustTowerDefenseApplication {
            graphics_state: GraphicsState::init(),
        }
    }

    /// Runs window state's event loop until a CloseRequested event is received
    /// This should take a event pipe to write keyboard events to that can
    /// be processed by other systems.
    fn main_loop(mut self) {
        let (frame_width, frame_height) = self.graphics_state.window_state.get_frame_size();
        let mut local_state = LocalState {
            frame_width,
            frame_height,
            mouse_x: 0.0,
            mouse_y: 0.0,
        };

        let mut events_loop = self
            .graphics_state
            .window_state
            .events_loop
            .take()
            .expect("events_loop does not exist!");

        loop {
            let inputs = UserInput::poll_events_loop(&mut events_loop);
            match inputs {
                Some(UserInput::EndRequested) => {
                    break;
                }
                Some(UserInput::NewFrameSize(_)) => {
                    debug!("Window changed size, restarting HalState...");
                    self.graphics_state.restart_halstate(local_state);
                }
                Some(r) => {
                    local_state.update_from_input(r);
                }
                None => (),
            };
            if let Err(e) = self.do_the_render(&local_state) {
                debug!("Rendering Error: {:?}", e);
                debug!("Auto-restarting HalState...");
                self.graphics_state.restart_halstate(local_state);
            }

            self.graphics_state.recreate_swapchain_if_necessary();
        }
    }

    fn do_the_render(&mut self, local_state: &LocalState) -> Result<(), &'static str> {
        let x = ((local_state.mouse_x / local_state.frame_width) * 2.0) - 1.0;
        let y = ((local_state.mouse_y / local_state.frame_height) * 2.0) - 1.0;
        let triangle = Triangle {
            points: [[-0.5, 0.5], [-0.5, -0.5], [x as f32, y as f32]],
        };

        self.graphics_state.draw_triangle(triangle)
    }

    /// Runs the application's main loop function.
    fn run(self) {
        info!("Running application...");
        self.main_loop();
    }
}
