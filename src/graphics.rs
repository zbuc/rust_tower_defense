#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx12",
        feature = "metal",
        feature = "gl"
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]

extern crate log;
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

use hal::{
    queue, Adapter, Backend, Capability, Features, Gpu, Graphics, Instance, PhysicalDevice,
    QueueFamily,
};
use winit::{dpi, ControlFlow, Event, EventsLoop, Window, WindowBuilder, WindowEvent};

use log::Level;

static WINDOW_NAME: &str = "Rust Tower Defense 0.1.0";

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
    let mut application = RustTowerDefenseApplication::init();
    application.run();
    info!("Cleaning up the application!");
    application.clean_up();
}

/// The state object for the window.
struct WindowState {
    events_loop: EventsLoop,
    _window: Window,
}

/// The state object for the graphics backend.
struct HalState {
    _command_queues: Vec<queue::CommandQueue<back::Backend, Graphics>>,
    _device: <back::Backend as Backend>::Device,
    _adapter: Adapter<back::Backend>,
    _instance: back::Instance,
}

impl HalState {
    /// No clean-up
    fn clean_up(self) {}
}

/// Hold a graphics family that has been selected for the graphics
/// display queue. This allows us to describe the queue features supported
/// by the graphics display device.
#[derive(Default)]
struct QueueFamilyIds {
    graphics_family: Option<queue::QueueFamilyId>,
}

impl QueueFamilyIds {
    /// Return whether the queue family is supported by the graphics device.
    fn is_complete(&self) -> bool {
        self.graphics_family.is_some()
    }
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

    /// Interfaces with the mutable physical adapter backend, providing
    /// a higher-level interface via command queues.
    fn create_device_with_graphics_queues(
        adapter: &mut Adapter<back::Backend>,
    ) -> (
        <back::Backend as Backend>::Device,
        Vec<queue::CommandQueue<back::Backend, Graphics>>,
    ) {
        let family = adapter
            .queue_families
            .iter()
            .find(|family| Graphics::supported_by(family.queue_type()) && family.max_queues() > 0)
            .expect("Could not find a queue family supporting graphics.");

        // we only want to create a single queue
        let priorities = vec![1.0; 1];
        let families = [(family, priorities.as_slice())];

        let Gpu { device, mut queues } = unsafe {
            adapter
                .physical_device
                .open(&families, Features::empty())
                .expect("Could not create device.")
        };

        let mut queue_group = queues
            .take::<Graphics>(family.id())
            .expect("Could not take ownership of relevant queue group.");

        let command_queues: Vec<_> = queue_group.queues.drain(..1).collect();

        (device, command_queues)
    }

    /// Creates a new instance of the graphics device's state
    fn init_hal() -> HalState {
        let instance = RustTowerDefenseApplication::create_device_instance();
        let mut adapter = RustTowerDefenseApplication::pick_adapter(&instance);
        let (device, command_queues) =
            RustTowerDefenseApplication::create_device_with_graphics_queues(&mut adapter);

        HalState {
            _command_queues: command_queues,
            _device: device,
            _adapter: adapter,
            _instance: instance,
        }
    }

    /// Finds the queue families supported by an Adapter, and returns
    /// the results in a QueueFamilyIds struct.
    fn find_queue_families(adapter: &Adapter<back::Backend>) -> QueueFamilyIds {
        let mut queue_family_ids = QueueFamilyIds::default();

        for queue_family in &adapter.queue_families {
            if queue_family.max_queues() > 0 && queue_family.supports_graphics() {
                queue_family_ids.graphics_family = Some(queue_family.id());
            }

            if queue_family_ids.is_complete() {
                break;
            }
        }

        queue_family_ids
    }

    /// Returns true if the adapter supports the necessary queue families
    /// for the application.
    fn is_adapter_suitable(adapter: &Adapter<back::Backend>) -> bool {
        RustTowerDefenseApplication::find_queue_families(adapter).is_complete()
    }

    /// Iterate every display adapter available to the display backend
    /// and select the first suitable adapter.
    fn pick_adapter(instance: &back::Instance) -> Adapter<back::Backend> {
        let adapters = instance.enumerate_adapters();
        for adapter in adapters {
            if RustTowerDefenseApplication::is_adapter_suitable(&adapter) {
                return adapter;
            }
        }
        panic!("No suitable adapter");
    }

    /// Runs window state's event loop until a CloseRequested event is received
    fn main_loop(&mut self) {
        info!("Starting event loop...");
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
        info!("Running application...");
        self.main_loop();
    }

    /// Disposes of the device's state, which may contain... stuff used by the graphics device.
    fn clean_up(self) {
        self.hal_state.clean_up();
    }
}