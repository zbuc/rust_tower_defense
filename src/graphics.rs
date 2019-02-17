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

use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::error::Error;
use std::fs::{File};
use std::io::{self,Read,Write};
use std::path::{Path, PathBuf};

use hal::{
    command, format, image, pass, pool, pso, queue, window, Adapter, Backbuffer, Backend,
    Capability, Device, Features, Gpu, Graphics, Instance, PhysicalDevice, Primitive, QueueFamily,
    Surface, Swapchain, SwapchainConfig,
};
use winit::{dpi, ControlFlow, Event, EventsLoop, Window, WindowBuilder, WindowEvent};

//use log::Level;

const MAX_FRAMES_IN_FLIGHT: usize = 2;
static WINDOW_NAME: &str = "Rust Tower Defense 0.1.0";
static SHADER_DIR: &str = "./assets/gen/shaders/";

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

/// Holds configuration flags for the window.
struct WindowConfig {
    is_maximized: bool,
    is_fullscreen: bool,
    decorations: bool,
    #[cfg(target_os = "macos")]
    macos_use_simple_fullscreen: bool,
}

/// The state object for the window.
struct WindowState {
    events_loop: Option<EventsLoop>,
    window: Window,
    window_config: RefCell<WindowConfig>,
}

/// The state object for the graphics backend.
struct HalState {
    in_flight_fences: Vec<<back::Backend as Backend>::Fence>,
    render_finished_semaphores: Vec<<back::Backend as Backend>::Semaphore>,
    image_available_semaphores: Vec<<back::Backend as Backend>::Semaphore>,
    submission_command_buffers:
        Vec<command::CommandBuffer<back::Backend, Graphics, command::MultiShot, command::Primary>>,
    command_pool: pool::CommandPool<back::Backend, Graphics>,
    swapchain_framebuffers: Vec<<back::Backend as Backend>::Framebuffer>,
    gfx_pipeline: <back::Backend as Backend>::GraphicsPipeline,
    descriptor_set_layouts: Vec<<back::Backend as Backend>::DescriptorSetLayout>,
    pipeline_layout: <back::Backend as Backend>::PipelineLayout,
    render_pass: <back::Backend as Backend>::RenderPass,
    frame_images: Vec<(
        <back::Backend as Backend>::Image,
        <back::Backend as Backend>::ImageView,
    )>,
    _format: format::Format,
    swapchain: <back::Backend as Backend>::Swapchain,
    command_queues: Vec<queue::CommandQueue<back::Backend, Graphics>>,
    device: <back::Backend as Backend>::Device,
    _surface: <back::Backend as Backend>::Surface,
    _adapter: Adapter<back::Backend>,
    _instance: back::Instance,
}

impl HalState {
    /// Clean up things on the graphics device's state, right now the
    /// render pipeline & layout need to be destroyed, the swapchain needs to
    /// be destroyed, and the frame images need to be destroyed.
    unsafe fn clean_up(self) {
        let device = &self.device;

        for fence in self.in_flight_fences {
            device.destroy_fence(fence)
        }

        for semaphore in self.render_finished_semaphores {
            device.destroy_semaphore(semaphore)
        }

        for semaphore in self.image_available_semaphores {
            device.destroy_semaphore(semaphore)
        }

        device.destroy_command_pool(self.command_pool.into_raw());

        for framebuffer in self.swapchain_framebuffers {
            device.destroy_framebuffer(framebuffer);
        }

        device.destroy_graphics_pipeline(self.gfx_pipeline);

        for descriptor_set_layout in self.descriptor_set_layouts {
            device.destroy_descriptor_set_layout(descriptor_set_layout);
        }

        device.destroy_pipeline_layout(self.pipeline_layout);

        device.destroy_render_pass(self.render_pass);

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

type ShaderData = Vec<u8>;

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

    // https://github.com/tomaka/winit/blob/master/examples/fullscreen.rs
    fn get_monitor(events_loop: &winit::EventsLoop) -> Option<winit::MonitorId> {
        #[cfg(target_os = "macos")]
        let mut macos_use_simple_fullscreen = false;

        // On macOS there are two fullscreen modes "native" and "simple"
        #[cfg(target_os = "macos")]
        {
            print!("Please choose the fullscreen mode: (1) native, (2) simple: ");
            io::stdout().flush().unwrap();

            let mut num = String::new();
            io::stdin().read_line(&mut num).unwrap();
            let num = num.trim().parse().ok().expect("Please enter a number");
            match num {
                2 => macos_use_simple_fullscreen = true,
                _ => {}
            }

            // Prompt for monitor when using native fullscreen
            if !macos_use_simple_fullscreen {
                Some(RustTowerDefenseApplication::prompt_for_monitor(&events_loop))
            } else {
                None
            }
        }

        #[cfg(not(target_os = "macos"))]
        Some(RustTowerDefenseApplication::prompt_for_monitor(&events_loop))
    }

    /// Enumerate monitors and prompt user to choose one
    fn prompt_for_monitor(events_loop: &winit::EventsLoop) -> winit::MonitorId {
        for (num, monitor) in events_loop.get_available_monitors().enumerate() {
            println!("Monitor #{}: {:?}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        io::stdout().flush().unwrap();

        let mut num = String::new();
        io::stdin().read_line(&mut num).unwrap();
        let num = num.trim().parse().ok().expect("Please enter a number");
        let monitor = events_loop.get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {:?}", monitor.get_name());

        monitor
    }

    /// Initializes the window state. Creates a new event loop, builds the window
    /// at the desired resolution, sets the title, and returns the newly created window
    /// with those parameters.
    fn init_window() -> WindowState {
        let events_loop = EventsLoop::new();

        let monitor = RustTowerDefenseApplication::get_monitor(&events_loop);

        let is_fullscreen = monitor.is_some();

        let window_builder = WindowBuilder::new()
            .with_fullscreen(monitor)
            .with_dimensions(dpi::LogicalSize::new(1024., 768.))
            .with_title(WINDOW_NAME.to_string());
        let window = window_builder.build(&events_loop).unwrap();

        let window_config = RefCell::new(WindowConfig {
            is_fullscreen: false,
            is_maximized: false,
            decorations: true,
            #[cfg(target_os = "macos")]
            macos_use_simple_fullscreen: false,
        });

        WindowState {
            events_loop: Some(events_loop),
            window: window,
            window_config,
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
        queue::QueueType,
        queue::family::QueueFamilyId,
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

        (device, command_queues, family.queue_type(), family.id())
    }

    /// Creates a new instance of the graphics device's state. This maintains
    /// everything necessary to set up the rendering pipeline from graphics device
    /// to window surface.
    unsafe fn init_hal(window: &Window) -> HalState {
        let instance = RustTowerDefenseApplication::create_device_instance();
        let mut adapter = RustTowerDefenseApplication::pick_adapter(&instance);
        let mut surface = RustTowerDefenseApplication::create_surface(&instance, window);
        let (device, command_queues, queue_type, qf_id) =
            RustTowerDefenseApplication::create_device_with_graphics_queues(&mut adapter, &surface);
        let (swapchain, extent, backbuffer, format) =
            RustTowerDefenseApplication::create_swap_chain(&adapter, &device, &mut surface, None);
        let frame_images =
            RustTowerDefenseApplication::create_image_views(backbuffer, format, &device);
        let render_pass = RustTowerDefenseApplication::create_render_pass(&device, Some(format));
        let (descriptor_set_layouts, pipeline_layout, gfx_pipeline) =
            RustTowerDefenseApplication::create_graphics_pipeline(&device, extent, &render_pass);
        let swapchain_framebuffers = RustTowerDefenseApplication::create_framebuffers(
            &device,
            &render_pass,
            &frame_images,
            extent,
        );
        let mut command_pool =
            RustTowerDefenseApplication::create_command_pool(&device, queue_type, qf_id);
        let submission_command_buffers = RustTowerDefenseApplication::create_command_buffers(
            &mut command_pool,
            &render_pass,
            &swapchain_framebuffers,
            extent,
            &gfx_pipeline,
        );
        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
            RustTowerDefenseApplication::create_sync_objects(&device);

        HalState {
            in_flight_fences,
            render_finished_semaphores,
            image_available_semaphores,
            submission_command_buffers,
            command_pool,
            swapchain_framebuffers,
            gfx_pipeline,
            descriptor_set_layouts,
            pipeline_layout,
            render_pass,
            frame_images,
            _format: format,
            swapchain,
            command_queues,
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
        window::Extent2D,
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
        let extent = swap_config.extent;
        let (swapchain, backbuffer) = unsafe {
            device
                .create_swapchain(surface, swap_config, previous_swapchain)
                .unwrap()
        };

        (swapchain, extent, backbuffer, format)
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

    /// Gets the compiled shader code from the SHADER_DIR
    pub fn get_shader_code(shader_name: &str) -> Result<ShaderData, Box<dyn Error>> {
        // I will probably want to use some human-readable JSON config for top-level
        // map configurations.
        let shader_path = Path::new(SHADER_DIR).join(shader_name);
        let mut shader_file = File::open(shader_path)?;
        let mut shader_data = Vec::<u8>::new();
        shader_file.read_to_end(&mut shader_data)?;

        Ok(shader_data)
    }

    /// Creates the RenderPass.
    /// Before we can finish creating the pipeline, we need to tell Vulkan about
    /// the framebuffer attachments that will be used while rendering. We need to
    /// specify how many color and depth buffers there will be, how many samples to
    /// use for each of them and how their contents should be handled throughout the
    /// rendering operations. All of this information is wrapped in a render pass object.
    fn create_render_pass(
        device: &<back::Backend as Backend>::Device,
        format: Option<format::Format>,
    ) -> <back::Backend as Backend>::RenderPass {
        // we're not doing anything with multisampling yet, so we'll stick to 1 sample.
        let samples: u8 = 1;

        let ops = pass::AttachmentOps {
            load: pass::AttachmentLoadOp::Clear,
            store: pass::AttachmentStoreOp::Store,
        };

        let stencil_ops = pass::AttachmentOps::DONT_CARE;

        let layouts = image::Layout::Undefined..image::Layout::Present;

        // In our case we'll have just a single color buffer attachment represented by one of the images from the swap chain.
        let color_attachment = pass::Attachment {
            format,
            samples,
            ops,
            stencil_ops,
            layouts,
        };

        let color_attachment_ref: pass::AttachmentRef = (0, image::Layout::ColorAttachmentOptimal);

        // hal assumes pipeline bind point is GRAPHICS
        let subpass = pass::SubpassDesc {
            colors: &[color_attachment_ref],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        };

        unsafe {
            // A single render pass can consist of multiple subpasses. Subpasses are subsequent
            // rendering operations that depend on the contents of framebuffers in previous passes,
            // for example a sequence of post-processing effects that are applied one after another.
            // If you group these rendering operations into one render pass, then Vulkan is able to
            // reorder the operations and conserve memory bandwidth for possibly better performance.
            // For our very first triangle, however, we'll stick to a single subpass.
            device
                .create_render_pass(&[color_attachment], &[subpass], &[])
                .unwrap()
        }
    }

    /// Creates the actual graphics pipeline, consisting of programmable (shader) elements as well
    /// as fixed functions.
    unsafe fn create_graphics_pipeline(
        device: &<back::Backend as Backend>::Device,
        extent: window::Extent2D,
        render_pass: &<back::Backend as Backend>::RenderPass,
    ) -> (
        Vec<<back::Backend as Backend>::DescriptorSetLayout>,
        <back::Backend as Backend>::PipelineLayout,
        <back::Backend as Backend>::GraphicsPipeline,
    ) {
        let vert_shader_code = RustTowerDefenseApplication::get_shader_code("test.vert.spv")
            .expect("Error loading vertex shader code.");

        let frag_shader_code = RustTowerDefenseApplication::get_shader_code("test.frag.spv")
            .expect("Error loading fragment shader code.");

        let vert_shader_module = device
            .create_shader_module(&vert_shader_code)
            .expect("Error creating shader module.");
        let frag_shader_module = device
            .create_shader_module(&frag_shader_code)
            .expect("Error creating fragment module.");

        let (ds_layouts, pipeline_layout, gfx_pipeline) = {
            let (vs_entry, fs_entry) = (
                pso::EntryPoint::<back::Backend> {
                    entry: "main",
                    module: &vert_shader_module,
                    specialization: hal::pso::Specialization {
                        constants: &[],
                        data: &[],
                    },
                },
                pso::EntryPoint::<back::Backend> {
                    entry: "main",
                    module: &frag_shader_module,
                    specialization: hal::pso::Specialization {
                        constants: &[],
                        data: &[],
                    },
                },
            );

            let shaders = pso::GraphicsShaderSet {
                vertex: vs_entry,
                hull: None,
                domain: None,
                geometry: None,
                fragment: Some(fs_entry),
            };

            let rasterizer = pso::Rasterizer {
                depth_clamping: false,
                polygon_mode: pso::PolygonMode::Fill,
                cull_face: <pso::Face>::BACK,
                front_face: pso::FrontFace::Clockwise,
                depth_bias: None,
                conservative: false,
            };

            // no need to set up vertex input format, as it is hardcoded
            let vertex_buffers: Vec<pso::VertexBufferDesc> = Vec::new();
            let attributes: Vec<pso::AttributeDesc> = Vec::new();

            let input_assembler = pso::InputAssemblerDesc::new(Primitive::TriangleList);

            // implements optional blending description provided in vulkan-tutorial
            // After a fragment shader has returned a color, it needs to be combined with
            // the color that is already in the framebuffer. This transformation is known as
            // color blending and there are two ways to do it:
            // Mix the old and new value to produce a final color
            // Combine the old and new value using a bitwise operation
            // Not really sure what we do here :)
            // I think it's hardcoded, and this differs from Vulkan to HAL
            let blender = {
                let blend_state = pso::BlendState::On {
                    color: pso::BlendOp::Add {
                        src: pso::Factor::One,
                        dst: pso::Factor::Zero,
                    },
                    alpha: pso::BlendOp::Add {
                        src: pso::Factor::One,
                        dst: pso::Factor::Zero,
                    },
                };

                pso::BlendDesc {
                    logic_op: Some(pso::LogicOp::Copy),
                    targets: vec![pso::ColorBlendDesc(pso::ColorMask::ALL, blend_state)],
                }
            };

            let depth_stencil = pso::DepthStencilDesc {
                depth: pso::DepthTest::Off,
                depth_bounds: false,
                stencil: pso::StencilTest::Off,
            };

            // No multisampling -- it could be used for antialiasing
            let multisampling: Option<pso::Multisampling> = None;

            // viewports and scissors
            let baked_states = pso::BakedStates {
                viewport: Some(pso::Viewport {
                    rect: pso::Rect {
                        x: 0,
                        y: 0,
                        w: extent.width as i16,
                        h: extent.height as i16,
                    },
                    depth: (0.0..1.0),
                }),
                scissor: Some(pso::Rect {
                    x: 0,
                    y: 0,
                    w: extent.width as i16,
                    h: extent.height as i16,
                }),
                blend_color: None,
                depth_bounds: None,
            };

            // with HAL, user only needs to specify whether overall state is static or dynamic

            // pipeline layout
            let bindings = Vec::<pso::DescriptorSetLayoutBinding>::new();
            let immutable_samplers = Vec::<<back::Backend as Backend>::Sampler>::new();
            let ds_layouts: Vec<<back::Backend as Backend>::DescriptorSetLayout> = vec![device
                .create_descriptor_set_layout(bindings, immutable_samplers)
                .unwrap()];
            let push_constants = Vec::<(pso::ShaderStageFlags, std::ops::Range<u32>)>::new();
            let layout = device
                .create_pipeline_layout(&ds_layouts, push_constants)
                .unwrap();

            let subpass = pass::Subpass {
                index: 0,
                main_pass: render_pass,
            };

            let flags = pso::PipelineCreationFlags::empty();

            let parent = pso::BasePipeline::None;

            let gfx_pipeline = {
                let desc = pso::GraphicsPipelineDesc {
                    shaders,
                    rasterizer,
                    vertex_buffers,
                    attributes,
                    input_assembler,
                    blender,
                    depth_stencil,
                    multisampling,
                    baked_states,
                    layout: &layout,
                    subpass,
                    flags,
                    parent,
                };

                device
                    .create_graphics_pipeline(&desc, None)
                    .expect("failed to create graphics pipeline!")
            };

            (ds_layouts, layout, gfx_pipeline)
        };

        device.destroy_shader_module(vert_shader_module);
        device.destroy_shader_module(frag_shader_module);

        (ds_layouts, pipeline_layout, gfx_pipeline)
    }

    /// Creates the framebuffers and attaches them to the device
    fn create_framebuffers(
        device: &<back::Backend as Backend>::Device,
        render_pass: &<back::Backend as Backend>::RenderPass,
        frame_images: &[(
            <back::Backend as Backend>::Image,
            <back::Backend as Backend>::ImageView,
        )],
        extent: window::Extent2D,
    ) -> Vec<<back::Backend as Backend>::Framebuffer> {
        let mut swapchain_framebuffers: Vec<<back::Backend as Backend>::Framebuffer> = Vec::new();

        unsafe {
            for (_, image_view) in frame_images.iter() {
                swapchain_framebuffers.push(
                    device
                        .create_framebuffer(
                            render_pass,
                            vec![image_view],
                            image::Extent {
                                width: extent.width as _,
                                height: extent.height as _,
                                depth: 1,
                            },
                        )
                        .expect("failed to create framebuffer!"),
                );
            }
        }

        swapchain_framebuffers
    }

    /// Commands in Vulkan, like drawing operations and memory transfers, are not executed
    /// directly using function calls. You have to record all of the operations you want to
    /// perform in command buffer objects. The advantage of this is that all of the hard work
    /// of setting up the drawing commands can be done in advance and in multiple threads. After
    /// that, you just have to tell Vulkan to execute the commands in the main loop.
    unsafe fn create_command_buffers<'a>(
        command_pool: &'a mut pool::CommandPool<back::Backend, Graphics>,
        render_pass: &<back::Backend as Backend>::RenderPass,
        framebuffers: &[<back::Backend as Backend>::Framebuffer],
        extent: window::Extent2D,
        pipeline: &<back::Backend as Backend>::GraphicsPipeline,
    ) -> Vec<command::CommandBuffer<back::Backend, Graphics, command::MultiShot, command::Primary>>
    {
        // pre-allocating memory primary command buffers is not necessary: HAL handles automatically

        let mut submission_command_buffers: Vec<
            command::CommandBuffer<back::Backend, Graphics, command::MultiShot, command::Primary>,
        > = Vec::new();

        for fb in framebuffers.iter() {
            // command buffer will be returned in 'recording' state
            // Shot: how many times a command buffer can be submitted; we want MultiShot (allow submission multiple times)
            // Level: command buffer type (primary or secondary)
            let mut command_buffer: command::CommandBuffer<
                back::Backend,
                Graphics,
                command::MultiShot,
                command::Primary,
            > = command_pool.acquire_command_buffer();

            // allow_pending_resubmit to be true, per vulkan-tutorial
            command_buffer.begin(true);
            command_buffer.bind_graphics_pipeline(pipeline);
            {
                // begin render pass
                let render_area = pso::Rect {
                    x: 0,
                    y: 0,
                    w: extent.width as _,
                    h: extent.height as _,
                };
                let clear_values = vec![command::ClearValue::Color(command::ClearColor::Float([
                    0.0, 0.0, 0.0, 1.0,
                ]))];

                let mut render_pass_inline_encoder = command_buffer.begin_render_pass_inline(
                    render_pass,
                    fb,
                    render_area,
                    clear_values.iter(),
                );
                // HAL encoder draw command is best understood by seeing how it expands out:
                // vertex_count = vertices.end - vertices.start
                // instance_count = instances.end - instances.start
                // first_vertex = vertices.start
                // first_instance = instances.start
                render_pass_inline_encoder.draw(0..3, 0..1);
            }

            command_buffer.finish();
            submission_command_buffers.push(command_buffer);
        }

        submission_command_buffers
    }

    /// We have to create a command pool before we can create command buffers.
    /// Command pools manage the memory that is used to store the buffers and
    /// command buffers are allocated from them.
    unsafe fn create_command_pool(
        device: &<back::Backend as Backend>::Device,
        queue_type: queue::QueueType,
        qf_id: queue::family::QueueFamilyId,
    ) -> pool::CommandPool<back::Backend, Graphics> {
        // raw command pool: a thin wrapper around command pools
        // strongly typed command pool: a safe wrapper around command pools, which ensures that only one command buffer is recorded at the same time from the current queue
        let raw_command_pool = device
            .create_command_pool(qf_id, pool::CommandPoolCreateFlags::empty())
            .unwrap();

        // safety check necessary before creating a strongly typed command pool
        assert_eq!(Graphics::supported_by(queue_type), true);
        pool::CommandPool::new(raw_command_pool)
    }

    /// The drawFrame function will perform the following operations:
    ///
    /// Acquire an image from the swap chain
    /// Execute the command buffer with that image as attachment in the framebuffer
    /// Return the image to the swap chain for presentation
    unsafe fn draw_frame(
        device: &<back::Backend as Backend>::Device,
        command_queues: &mut [queue::CommandQueue<back::Backend, Graphics>],
        swapchain: &mut <back::Backend as Backend>::Swapchain,
        submission_command_buffers: &[command::CommandBuffer<
            back::Backend,
            Graphics,
            command::MultiShot,
            command::Primary,
        >],
        image_available_semaphore: &<back::Backend as Backend>::Semaphore,
        render_finished_semaphore: &<back::Backend as Backend>::Semaphore,
        in_flight_fence: &<back::Backend as Backend>::Fence,
    ) {
        device
            .wait_for_fence(in_flight_fence, std::u64::MAX)
            .unwrap();
        device.reset_fence(in_flight_fence).unwrap();

        let image_index = swapchain
            .acquire_image(
                std::u64::MAX,
                window::FrameSync::Semaphore(image_available_semaphore),
            )
            .expect("could not acquire image!");

        let i = image_index as usize;
        let submission = queue::Submission {
            command_buffers: &submission_command_buffers[i..i + 1],
            wait_semaphores: vec![(
                image_available_semaphore,
                pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            )],
            signal_semaphores: vec![render_finished_semaphore],
        };

        // recall we only made one queue
        command_queues[0].submit(submission, Some(in_flight_fence));

        swapchain
            .present(
                &mut command_queues[0],
                image_index,
                vec![render_finished_semaphore],
            )
            .expect("presentation failed!");
    }

    fn create_sync_objects(
        device: &<back::Backend as Backend>::Device,
    ) -> (
        Vec<<back::Backend as Backend>::Semaphore>,
        Vec<<back::Backend as Backend>::Semaphore>,
        Vec<<back::Backend as Backend>::Fence>,
    ) {
        let mut image_available_semaphores: Vec<<back::Backend as Backend>::Semaphore> = Vec::new();
        let mut render_finished_semaphores: Vec<<back::Backend as Backend>::Semaphore> = Vec::new();
        let mut in_flight_fences: Vec<<back::Backend as Backend>::Fence> = Vec::new();

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available_semaphores.push(device.create_semaphore().unwrap());
            render_finished_semaphores.push(device.create_semaphore().unwrap());
            in_flight_fences.push(device.create_fence(true).unwrap());
        }

        (
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
        )
    }

    /// Runs window state's event loop until a CloseRequested event is received
    /// This should take a event pipe to write keyboard events to that can
    /// be processed by other systems.
    fn main_loop(&mut self) {
        let mut current_frame: usize = 0;

        let mut events_loop = self
            .window_state
            .events_loop
            .take()
            .expect("events_loop does not exist!");
        events_loop.run_forever(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                self.hal_state
                    .device
                    .wait_idle()
                    .expect("Queues are not going idle!");
                ControlFlow::Break
            }
            _ => {
                unsafe {
                    RustTowerDefenseApplication::draw_frame(
                        &self.hal_state.device,
                        &mut self.hal_state.command_queues,
                        &mut self.hal_state.swapchain,
                        &self.hal_state.submission_command_buffers,
                        &self.hal_state.image_available_semaphores[current_frame],
                        &self.hal_state.render_finished_semaphores[current_frame],
                        &self.hal_state.in_flight_fences[current_frame],
                    );
                }

                current_frame = (current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
                ;
                ControlFlow::Continue
            }
        });
        self.window_state.events_loop = Some(events_loop);
    }
    // fn main_loop(&mut self) {
    //     info!("Starting event loop...");
    //     let mut current_frame: usize = 0;

    //     let mut window_config = self.window_state.window_config.borrow_mut();
    //     self.window_state
    //         .events_loop.borrow_mut()
    //         .run_forever(|event| {
    //             println!("{:?}", event);

    //             match event {
    //                 Event::WindowEvent { event, .. } => match event {
    //                     WindowEvent::CloseRequested => {
    //                         // self.hal_state
    //                         //     .device
    //                         //     .wait_idle()
    //                         //     .expect("Queues are not going idle!");
    //                         return ControlFlow::Break;
    //                     },
    //                     WindowEvent::KeyboardInput {
    //                         input:
    //                             winit::KeyboardInput {
    //                                 virtual_keycode: Some(virtual_code),
    //                                 state,
    //                                 ..
    //                             },
    //                         ..
    //                     } => match (virtual_code, state) {
    //                         (winit::VirtualKeyCode::Escape, _) => {
    //                             // self.hal_state
    //                             //     .device
    //                             //     .wait_idle()
    //                             //     .expect("Queues are not going idle!");

    //                             return ControlFlow::Break;
    //                         },
    //                         (winit::VirtualKeyCode::F, winit::ElementState::Pressed) => {
    //                             #[cfg(target_os = "macos")]
    //                             {
    //                                 if window_config.macos_use_simple_fullscreen {
    //                                     use winit::os::macos::WindowExt;
    //                                     if WindowExt::set_simple_fullscreen(&self.window_state.window, !window_config.is_fullscreen) {
    //                                         window_config.is_fullscreen = !window_config.is_fullscreen;
    //                                     }

    //                                     return ControlFlow::Continue;
    //                                 }
    //                             }

    //                             window_config.is_fullscreen = !window_config.is_fullscreen;
    //                             if !window_config.is_fullscreen {
    //                                 self.window_state.window.set_fullscreen(None);
    //                             } else {
    //                                 self.window_state.window.set_fullscreen(Some(self.window_state.window.get_current_monitor()));
    //                             }
    //                         }
    //                         (winit::VirtualKeyCode::M, winit::ElementState::Pressed) => {
    //                             window_config.is_maximized = !window_config.is_maximized;
    //                             self.window_state.window.set_maximized(window_config.is_maximized);
    //                         }
    //                         (winit::VirtualKeyCode::D, winit::ElementState::Pressed) => {
    //                             window_config.decorations = !window_config.decorations;
    //                             self.window_state.window.set_decorations(window_config.decorations);
    //                         }
    //                         _ => (),
    //                     },
    //                     _ => (),
    //                 },
    //                 _ => {}
    //             }

    //             unsafe {
    //                 RustTowerDefenseApplication::draw_frame(
    //                     &self.hal_state.device,
    //                     &mut self.hal_state.command_queues,
    //                     &mut self.hal_state.swapchain,
    //                     &self.hal_state.submission_command_buffers,
    //                     &self.hal_state.image_available_semaphores[current_frame],
    //                     &self.hal_state.render_finished_semaphores[current_frame],
    //                     &self.hal_state.in_flight_fences[current_frame],
    //                 );
    //             }

    //             current_frame = (current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    //             ;
    //             ControlFlow::Continue
    //         });
    // }

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
