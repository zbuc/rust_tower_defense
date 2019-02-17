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
extern crate log;

extern crate glsl_to_spirv;
extern crate winit;

use hal::{
    format, image, queue, window, Adapter, Backbuffer, Backend, Capability, Device, Features, Gpu,
    Graphics, Instance, PhysicalDevice, QueueFamily, Surface, SwapchainConfig,
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
    unsafe {
        application.clean_up();
    }
}

/// The state object for the window.
struct WindowState {
    events_loop: EventsLoop,
    window: Window,
}

/// The state object for the graphics backend.
struct HalState {
    frame_images: Vec<(
        <back::Backend as Backend>::Image,
        <back::Backend as Backend>::ImageView,
    )>,
    _format: format::Format,
    swapchain: <back::Backend as Backend>::Swapchain,
    _command_queues: Vec<queue::CommandQueue<back::Backend, Graphics>>,
    device: <back::Backend as Backend>::Device,
    _surface: <back::Backend as Backend>::Surface,
    _adapter: Adapter<back::Backend>,
    _instance: back::Instance,
}

impl HalState {
    /// Clean up things on the graphics device's state, right now the
    /// swapchain needs to be destroyed, and the frame images need to
    /// be destroyed.
    unsafe fn clean_up(self) {
        let device = &self.device;

        for (_, image_view) in self.frame_images.into_iter() {
            device.destroy_image_view(image_view);
        }

        device.destroy_swapchain(self.swapchain);
    }
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
        let hal_state = unsafe { RustTowerDefenseApplication::init_hal(&window_state.window) };

        RustTowerDefenseApplication {
            hal_state,
            window_state,
        }
    }

    /// Creates a new instance of whatever graphics backend has been selected.
    fn create_device_instance() -> back::Instance {
        back::Instance::create(WINDOW_NAME, 1)
    }

    /// Creates a surface on the provided window that displays from the provided
    /// graphics backend instance.
    fn create_surface(
        instance: &back::Instance,
        window: &Window,
    ) -> <back::Backend as Backend>::Surface {
        instance.create_surface(window)
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
            window: window,
        }
    }

    /// Interfaces with the mutable physical adapter backend, providing
    /// a higher-level interface via command queues. Checks to see that
    /// the queue family supports both graphics and working with the provided
    /// surface.
    fn create_device_with_graphics_queues(
        adapter: &mut Adapter<back::Backend>,
        surface: &<back::Backend as Backend>::Surface,
    ) -> (
        <back::Backend as Backend>::Device,
        Vec<queue::CommandQueue<back::Backend, Graphics>>,
    ) {
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                Graphics::supported_by(family.queue_type())
                    && family.max_queues() > 0
                    && surface.supports_queue_family(family)
            })
            .expect("Could not find a queue family supporting graphics.");

        // we only want to create a single queue
        // https://vulkan-tutorial.com/Drawing_a_triangle/Setup/Logical_device_and_queues
        // The currently available drivers will only allow you to create a
        // small number of queues for each queue family and you don't really
        // need more than one. That's because you can create all of the command
        // buffers on multiple threads and then submit them all at once on the
        // main thread with a single low-overhead call.
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

    /// Creates a new instance of the graphics device's state. This maintains
    /// everything necessary to set up the rendering pipeline from graphics device
    /// to window surface.
    unsafe fn init_hal(window: &Window) -> HalState {
        let instance = RustTowerDefenseApplication::create_device_instance();
        let mut adapter = RustTowerDefenseApplication::pick_adapter(&instance);
        let mut surface = RustTowerDefenseApplication::create_surface(&instance, window);
        let (device, command_queues) =
            RustTowerDefenseApplication::create_device_with_graphics_queues(&mut adapter, &surface);
        let (swapchain, backbuffer, format) =
            RustTowerDefenseApplication::create_swap_chain(&adapter, &device, &mut surface, None);
        let frame_images =
            RustTowerDefenseApplication::create_image_views(backbuffer, format, &device);

        HalState {
            frame_images,
            _format: format,
            swapchain,
            _command_queues: command_queues,
            device,
            _surface: surface,
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

    /// Creates the swapchain, which represents a queue of images waiting to
    /// be presented to the screen.
    fn create_swap_chain(
        adapter: &Adapter<back::Backend>,
        device: &<back::Backend as Backend>::Device,
        surface: &mut <back::Backend as Backend>::Surface,
        previous_swapchain: Option<<back::Backend as Backend>::Swapchain>,
    ) -> (
        <back::Backend as Backend>::Swapchain,
        Backbuffer<back::Backend>,
        format::Format,
    ) {
        let (caps, formats, _present_modes) =
            surface.compatibility(&adapter.physical_device);

        // SRGB has "more accurate perceived colors", but
        // working directly with SRGB is annoying, so we map from RGB to SRGB.
        let format = formats.map_or(format::Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == format::ChannelType::Srgb)
                .map(|format| *format)
                .unwrap_or(formats[0])
        });

        // what should default extent be?
        let swap_config = SwapchainConfig::from_caps(&caps, format, caps.extents.end);
        let (swapchain, backbuffer) = unsafe {
            device
                .create_swapchain(surface, swap_config, previous_swapchain)
                .unwrap()
        };

        (swapchain, backbuffer, format)
    }

    /// An image view is quite literally a view into an image. It describes how to
    /// access the image and which part of the image to access, for example if it
    /// should be treated as a 2D texture depth texture without any mipmapping levels.
    /// 
    /// Creates a basic image view for every image in the swap chain's backbuffer so 
    /// that we can use them as color targets later on.
    /// https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Image_views
    unsafe fn create_image_views(
        backbuffer: Backbuffer<back::Backend>,
        format: format::Format,
        device: &<back::Backend as Backend>::Device,
    ) -> Vec<(
        <back::Backend as Backend>::Image,
        <back::Backend as Backend>::ImageView,
    )> {
        match backbuffer {
            window::Backbuffer::Images(images) => images
                .into_iter()
                .map(|image| {
                    let image_view = match device.create_image_view(
                        &image,
                        image::ViewKind::D2,
                        format,
                        format::Swizzle::NO,
                        image::SubresourceRange {
                            aspects: format::Aspects::COLOR,
                            levels: 0..1,
                            layers: 0..1,
                        },
                    ) {
                        Ok(image_view) => image_view,
                        Err(_) => panic!("Error creating image view for an image!"),
                    };

                    (image, image_view)
                })
                .collect(),
            // OpenGL case, where backbuffer is a framebuffer, not implemented currently
            _ => unimplemented!(),
        }
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
    unsafe fn clean_up(self) {
        self.hal_state.clean_up();
    }
}
