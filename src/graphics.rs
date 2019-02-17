#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx12",
        feature = "metal",
        feature = "gl"
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

extern crate env_logger;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;

extern crate glsl_to_spirv;
extern crate image;
extern crate winit;

use winit::{dpi, ControlFlow, Event, EventsLoop, Window, WindowBuilder, WindowEvent};

static WINDOW_NAME: &str = "Rust Tower Defense 0.1.0";

/// Runs the main graphics event loop.
/// 
/// Exits when the window is closed.
/// Eventually we'll have a command system responsible
/// for telling the window to close.
pub fn run() {
    let mut application = RustTowerDefenseApplication::init();
    application.run();
    application.clean_up();
}

/// The state object for the window.
struct WindowState {
    events_loop: EventsLoop,
    _window: Window,
}

/// The state object for the graphics backend.
struct HalState {
    _instance: back::Instance,
}

impl HalState {
    /// No clean-up
    fn clean_up(self) {}
}

/// The application contains the window and graphics
/// drivers' states. This is the main interface to interact
/// with the application. We would probably expand this to be
/// the systems' manager at some point, or reference it from
/// the systems' manager.
struct RustTowerDefenseApplication {
    hal_state: HalState,
    window_state: WindowState,
}

impl RustTowerDefenseApplication {
    /// Initialize a new instance of the application. Will initialize the
    /// window and graphics state, and then return a new RustTowerDefenseApplication.
    pub fn init() -> RustTowerDefenseApplication {
        let window_state = RustTowerDefenseApplication::init_window();
        let hal_state = RustTowerDefenseApplication::init_hal();

        RustTowerDefenseApplication {
            hal_state,
            window_state,
        }
    }

    /// Creates a new instance of whatever graphics backend has been selected.
    fn create_device_instance() -> back::Instance {
        back::Instance::create(WINDOW_NAME, 1)
    }

    /// Initializes the window state. Creates a new event loop, builds the window
    /// at the desired resolution, sets the title, and returns the newly created window
    /// with those parameters.
    fn init_window() -> WindowState {
        let events_loop = EventsLoop::new();
        let window_builder = WindowBuilder::new()
            .with_dimensions(dpi::LogicalSize::new(1024., 768.))
            .with_title(WINDOW_NAME.to_string());
        let window = window_builder.build(&events_loop).unwrap();
        WindowState {
            events_loop,
            _window: window,
        }
    }

    /// Creates a new instance of the graphics device's state
    fn init_hal() -> HalState {
        let instance = RustTowerDefenseApplication::create_device_instance();
        HalState {
            _instance: instance,
        }
    }

    /// Runs window state's event loop until a CloseRequested event is received
    fn main_loop(&mut self) {
        self.window_state
            .events_loop
            .run_forever(|event| match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => ControlFlow::Break,
                _ => ControlFlow::Continue,
            });
    }

    /// Runs the application's main loop function.
    fn run(&mut self) {
        self.main_loop();
    }

    /// Disposes of the device's state, which may contain... stuff used by the graphics device.
    fn clean_up(self) {
        self.hal_state.clean_up();
    }
}