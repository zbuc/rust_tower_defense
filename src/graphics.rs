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
extern crate gfx_backend_vulkan as back;
extern crate log;

extern crate glsl_to_spirv;
extern crate winit;

use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::thread;

use crate::hal::{
    adapter::{Adapter, MemoryTypeId, PhysicalDevice},
    buffer::Usage as BufferUsage,
    command::{ClearColor, ClearValue, CommandBuffer, MultiShot, Primary},
    device::Device,
    format::{Aspects, ChannelType, Format, Swizzle},
    image::{Extent, Layout, SubresourceRange, Usage, ViewKind},
    memory::{Properties, Requirements},
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, Subpass, SubpassDesc},
    pool::{CommandPool, CommandPoolCreateFlags},
    pso::{
        AttributeDesc, BakedStates, BasePipeline, BlendDesc, BlendOp, BlendState, ColorBlendDesc,
        ColorMask, DepthStencilDesc, DepthTest, DescriptorSetLayoutBinding, Element, EntryPoint,
        Face, Factor, FrontFace, GraphicsPipelineDesc, GraphicsShaderSet, InputAssemblerDesc,
        LogicOp, PipelineCreationFlags, PipelineStage, PolygonMode, Rasterizer, Rect,
        ShaderStageFlags, Specialization, StencilTest, VertexBufferDesc, VertexInputRate, Viewport,
    },
    queue::{self, family::QueueGroup, Submission},
    window::{Backbuffer, Extent2D, PresentMode, Swapchain, SwapchainConfig},
    Backend, Gpu, Graphics, Instance, Primitive, QueueFamily, Surface,
};
use arrayvec::ArrayVec;
use core::mem::{size_of, ManuallyDrop};
use winit::dpi::LogicalSize;
use winit::{dpi, Event, EventsLoop, Window, WindowBuilder, WindowEvent};

//use log::Level;

pub const VERTEX_SOURCE: &str = "#version 450
layout (location = 0) in vec2 position;

out gl_PerVertex {
  vec4 gl_Position;
};

void main()
{
  gl_Position = vec4(position, 0.0, 1.0);
}";

pub const FRAGMENT_SOURCE: &str = "#version 450
layout(location = 0) out vec4 color;

void main()
{
  color = vec4(1.0);
}";
const MAX_FRAMES_IN_FLIGHT: usize = 2;
static WINDOW_NAME: &str = "Rust Tower Defense 0.1.0";
static SHADER_DIR: &str = "./assets/gen/shaders/";

/// Holds configuration flags for the window.
struct WindowConfig {
    is_maximized: bool,
    is_fullscreen: bool,
    decorations: bool,
    #[cfg(target_os = "macos")]
    macos_use_simple_fullscreen: bool,
}

/// The state object for the window.
pub struct WindowState {
    pub events_loop: Option<EventsLoop>,
    window: Window,
    window_config: RefCell<WindowConfig>,
}

impl WindowState {
    pub fn get_frame_size(&self) -> (f64, f64) {
        self.window
            .get_inner_size()
            .map(|logical| logical.into())
            .unwrap_or((0.0, 0.0))
    }
}

/// The state object for the graphics backend.
pub struct HalState {
    buffer: ManuallyDrop<<back::Backend as Backend>::Buffer>,
    memory: ManuallyDrop<<back::Backend as Backend>::Memory>,
    descriptor_set_layouts: Vec<<back::Backend as Backend>::DescriptorSetLayout>,
    pipeline_layout: ManuallyDrop<<back::Backend as Backend>::PipelineLayout>,
    graphics_pipeline: ManuallyDrop<<back::Backend as Backend>::GraphicsPipeline>,
    requirements: Requirements,
    current_frame: usize,
    frames_in_flight: usize,
    in_flight_fences: Vec<<back::Backend as Backend>::Fence>,
    render_finished_semaphores: Vec<<back::Backend as Backend>::Semaphore>,
    image_available_semaphores: Vec<<back::Backend as Backend>::Semaphore>,
    command_buffers: Vec<CommandBuffer<back::Backend, Graphics, MultiShot, Primary>>,
    command_pool: ManuallyDrop<CommandPool<back::Backend, Graphics>>,
    framebuffers: Vec<<back::Backend as Backend>::Framebuffer>,
    image_views: Vec<(<back::Backend as Backend>::ImageView)>,
    render_pass: ManuallyDrop<<back::Backend as Backend>::RenderPass>,
    render_area: Rect,
    queue_group: QueueGroup<back::Backend, Graphics>,
    swapchain: ManuallyDrop<<back::Backend as Backend>::Swapchain>,
    device: ManuallyDrop<back::Device>,
    adapter: Adapter<back::Backend>,
    surface: <back::Backend as Backend>::Surface,
    _instance: ManuallyDrop<back::Instance>,
    should_recreate_swapchain: bool,
    format: Format,
}

impl core::ops::Drop for HalState {
    /// We have to clean up "leaf" elements before "root" elements. Basically, we
    /// clean up in reverse of the order that we created things.
    fn drop(&mut self) {
        let _ = self.device.wait_idle();
        unsafe {
            for descriptor_set_layout in self.descriptor_set_layouts.drain(..) {
                self.device
                    .destroy_descriptor_set_layout(descriptor_set_layout)
            }
            for fence in self.in_flight_fences.drain(..) {
                self.device.destroy_fence(fence)
            }
            for semaphore in self.render_finished_semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore)
            }
            for semaphore in self.image_available_semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore)
            }
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer);
            }
            for image_view in self.image_views.drain(..) {
                self.device.destroy_image_view(image_view);
            }
            // LAST RESORT STYLE CODE, NOT TO BE IMITATED LIGHTLY
            use core::ptr::read;
            self.device
                .destroy_buffer(ManuallyDrop::into_inner(read(&self.buffer)));
            self.device
                .free_memory(ManuallyDrop::into_inner(read(&self.memory)));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));
            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.graphics_pipeline)));
            self.device.destroy_command_pool(
                ManuallyDrop::into_inner(read(&self.command_pool)).into_raw(),
            );
            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(read(&self.render_pass)));
            self.device
                .destroy_swapchain(ManuallyDrop::into_inner(read(&self.swapchain)));
            self.command_buffers = vec![];
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self._instance);
        }
    }
}

pub struct GraphicsState {
    pub hal_state: HalState,
    pub window_state: WindowState,
}

impl GraphicsState {
    pub fn init() -> Self {
        let window_state = Self::init_window();
        let hal_state = Self::init_hal_state(&window_state);

        Self {
            hal_state: hal_state,
            window_state: window_state,
        }
    }

    fn init_hal_state(window_state: &WindowState) -> HalState {
        let hal_state = match HalState::new(&window_state.window) {
            Ok(state) => state,
            Err(e) => panic!(e),
        };
        hal_state
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
                debug!("No simple fullscreen");
                Some(RustTowerDefenseApplication::prompt_for_monitor(
                    &events_loop,
                ))
            } else {
                debug!("Yes simple fullscreen");
                None
            }
        }

        #[cfg(not(target_os = "macos"))]
        Some(Self::prompt_for_monitor(&events_loop))
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
        let monitor = events_loop
            .get_available_monitors()
            .nth(num)
            .expect("Please enter a valid ID");

        println!("Using {:?}", monitor.get_name());

        monitor
    }
    /// Initializes the window state. Creates a new event loop, builds the window
    /// at the desired resolution, sets the title, and returns the newly created window
    /// with those parameters.
    ///
    /// ## Failure
    /// It's possible for the window creation to fail.
    fn init_window() -> WindowState {
        let tid = thread::current().id();
        debug!("thread ID (init_window) {:#?}", tid);
        let events_loop = EventsLoop::new();

        let monitor = Self::get_monitor(&events_loop);

        debug!("Monitor: {:#?}", monitor);
        let window_builder = WindowBuilder::new()
            .with_fullscreen(monitor)
            .with_dimensions(dpi::LogicalSize::new(2880., 1800.))
            .with_resizable(false)
            .with_title(WINDOW_NAME.to_string());
        let window = window_builder.build(&events_loop).unwrap();
        window.set_maximized(true);
        window.set_fullscreen(Some(window.get_current_monitor()));

        let window_config = RefCell::new(WindowConfig {
            is_fullscreen: false,
            is_maximized: true,
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

    pub fn restart_halstate(&mut self, _local_state: LocalState) {
        // XXX This actually isn't a great implementation, we want to keep
        // what state we can, but this is easier to implement.
        //let hal_state = HalState::new(&self.window_state.window).unwrap();
        //drop(self.hal_state);
        //mem::replace(&mut self.hal_state, hal_state);
        let tid = thread::current().id();
        debug!("thread ID (events_loop1) {:#?}", tid);
        self.hal_state.should_recreate_swapchain = true;
        // drop(self.hal_state);
        // self.hal_state = HalState::new(&self.window_state.window).unwrap();
    }

    pub fn draw_triangle(&mut self, triangle: Triangle) -> Result<(), &'static str> {
        self.hal_state.borrow_mut().draw_triangle_frame(triangle)
    }

    pub fn recreate_swapchain_if_necessary(&mut self) {
        if !self.hal_state.should_recreate_swapchain {
            return;
        }

        debug!("Have to recreate swapchain...");
        self.hal_state.device.wait_idle().unwrap();

        let (caps, formats, _present_modes) = self
            .hal_state
            .surface
            .compatibility(&mut self.hal_state.adapter.physical_device);
        // Verify that previous format still exists so we may reuse it.
        assert!(formats.iter().any(|fs| fs.contains(&self.hal_state.format)));

        // XXX TODO change this hardcoded dimension so we can support resize, see
        // https://github.com/gfx-rs/gfx/blob/afd03b752ca2a9d24b65de8ef293d58fd52ee4f2/examples/quad/main.rs
        // let swapchain_config = SwapchainConfig {
        //     present_mode,
        //     composite_alpha,
        //     format,
        //     extent,
        //     image_count,
        //     image_layers,
        //     image_usage,
        // };
        let extent = {
            Extent2D {
                width: 2880,
                height: 1800,
            }
        };
        let swap_config = SwapchainConfig::from_caps(&caps, self.hal_state.format, extent);
        println!("{:?}", swap_config);
        let extent = swap_config.extent.to_extent();

        let (new_swap_chain, new_backbuffer) = unsafe {
            self.hal_state.device.create_swapchain(
                &mut self.hal_state.surface,
                swap_config,
                None,
                // XXX TODO not sure what this implies, auto-drop?
                //Some(self.hal_state.swapchain),
            )
        }
        .expect("Can't create swapchain");

        unsafe {
            // Clean up the old framebuffers, images and swapchain
            &self.hal_state.cleanup_framebuffers();
        }

        // XXX TODO IDK??? do i need to drop this? it's manuallydrop
        //drop(self.hal_state.swapchain);
        self.hal_state.swapchain = ManuallyDrop::new(new_swap_chain);

        let (new_frame_images, new_framebuffers) = match new_backbuffer {
            Backbuffer::Images(images) => {
                let imageviews = images
                    .into_iter()
                    .map(|image| unsafe {
                        let rtv = self
                            .hal_state
                            .device
                            .create_image_view(
                                &image,
                                ViewKind::D2,
                                self.hal_state.format,
                                Swizzle::NO,
                                SubresourceRange {
                                    aspects: Aspects::COLOR,
                                    levels: 0..1,
                                    layers: 0..1,
                                },
                            )
                            .unwrap();
                        rtv
                    })
                    .collect::<Vec<_>>();
                let fbos = imageviews
                    .iter()
                    .map(|&ref rtv| unsafe {
                        self.hal_state
                            .device
                            .create_framebuffer(&self.hal_state.render_pass, Some(rtv), extent)
                            .unwrap()
                    })
                    .collect();
                (imageviews, fbos)
            }
            Backbuffer::Framebuffer(fbo) => (Vec::new(), vec![fbo]),
        };

        self.hal_state.framebuffers = new_framebuffers;
        self.hal_state.image_views = new_frame_images;
        // XXX TODO for resize vvv
        // self.hal_state.viewport.rect.w = extent.width as _;
        // self.hal_state.viewport.rect.h = extent.height as _;
        self.hal_state.should_recreate_swapchain = false;
    }
}

#[allow(clippy::type_complexity)]
fn create_pipeline(
    device: &mut back::Device,
    extent: Extent2D,
    render_pass: &<back::Backend as Backend>::RenderPass,
) -> Result<
    (
        Vec<<back::Backend as Backend>::DescriptorSetLayout>,
        <back::Backend as Backend>::PipelineLayout,
        <back::Backend as Backend>::GraphicsPipeline,
    ),
    &'static str,
> {
    let mut compiler = shaderc::Compiler::new().ok_or("shaderc not found!")?;
    let vertex_compile_artifact = compiler
        .compile_into_spirv(
            VERTEX_SOURCE,
            shaderc::ShaderKind::Vertex,
            "vertex.vert",
            "main",
            None,
        )
        .map_err(|_| "Couldn't compile vertex shader!")?;
    let fragment_compile_artifact = compiler
        .compile_into_spirv(
            FRAGMENT_SOURCE,
            shaderc::ShaderKind::Fragment,
            "fragment.frag",
            "main",
            None,
        )
        .map_err(|e| {
            error!("{}", e);
            "Couldn't compile fragment shader!"
        })?;
    let vertex_shader_module = unsafe {
        device
            .create_shader_module(vertex_compile_artifact.as_binary_u8())
            .map_err(|_| "Couldn't make the vertex module")?
    };
    let fragment_shader_module = unsafe {
        device
            .create_shader_module(fragment_compile_artifact.as_binary_u8())
            .map_err(|_| "Couldn't make the fragment module")?
    };
    let (descriptor_set_layouts, pipeline_layout, gfx_pipeline) = {
        let (vs_entry, fs_entry) = (
            EntryPoint {
                entry: "main",
                module: &vertex_shader_module,
                specialization: Specialization {
                    constants: &[],
                    data: &[],
                },
            },
            EntryPoint {
                entry: "main",
                module: &fragment_shader_module,
                specialization: Specialization {
                    constants: &[],
                    data: &[],
                },
            },
        );
        let shaders = GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };

        let input_assembler = InputAssemblerDesc::new(Primitive::TriangleList);

        let vertex_buffers: Vec<VertexBufferDesc> = vec![VertexBufferDesc {
            binding: 0,
            stride: (size_of::<f32>() * 2) as u32,
            rate: VertexInputRate::Vertex,
        }];
        let attributes: Vec<AttributeDesc> = vec![AttributeDesc {
            location: 0,
            binding: 0,
            element: Element {
                format: Format::Rg32Sfloat,
                offset: 0,
            },
        }];

        let rasterizer = Rasterizer {
            depth_clamping: false,
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::NONE,
            front_face: FrontFace::Clockwise,
            depth_bias: None,
            conservative: false,
        };

        let depth_stencil = DepthStencilDesc {
            depth: DepthTest::Off,
            depth_bounds: false,
            stencil: StencilTest::Off,
        };

        let blender = {
            let blend_state = BlendState::On {
                color: BlendOp::Add {
                    src: Factor::One,
                    dst: Factor::Zero,
                },
                alpha: BlendOp::Add {
                    src: Factor::One,
                    dst: Factor::Zero,
                },
            };
            BlendDesc {
                logic_op: Some(LogicOp::Copy),
                targets: vec![ColorBlendDesc(ColorMask::ALL, blend_state)],
            }
        };

        let baked_states = BakedStates {
            viewport: Some(Viewport {
                rect: extent.to_extent().rect(),
                depth: (0.0..1.0),
            }),
            scissor: Some(extent.to_extent().rect()),
            blend_color: None,
            depth_bounds: None,
        };

        let bindings = Vec::<DescriptorSetLayoutBinding>::new();
        let immutable_samplers = Vec::<<back::Backend as Backend>::Sampler>::new();
        let descriptor_set_layouts: Vec<<back::Backend as Backend>::DescriptorSetLayout> =
            vec![unsafe {
                device
                    .create_descriptor_set_layout(bindings, immutable_samplers)
                    .map_err(|_| "Couldn't make a DescriptorSetLayout")?
            }];
        let push_constants = Vec::<(ShaderStageFlags, core::ops::Range<u32>)>::new();
        let layout = unsafe {
            device
                .create_pipeline_layout(&descriptor_set_layouts, push_constants)
                .map_err(|_| "Couldn't create a pipeline layout")?
        };

        let gfx_pipeline = {
            let desc = GraphicsPipelineDesc {
                shaders,
                rasterizer,
                vertex_buffers,
                attributes,
                input_assembler,
                blender,
                depth_stencil,
                multisampling: None,
                baked_states,
                layout: &layout,
                subpass: Subpass {
                    index: 0,
                    main_pass: render_pass,
                },
                flags: PipelineCreationFlags::empty(),
                parent: BasePipeline::None,
            };

            unsafe {
                device
                    .create_graphics_pipeline(&desc, None)
                    .map_err(|_| "Couldn't create a graphics pipeline!")?
            }
        };

        (descriptor_set_layouts, layout, gfx_pipeline)
    };

    unsafe {
        device.destroy_shader_module(vertex_shader_module);
        device.destroy_shader_module(fragment_shader_module);
    }

    Ok((descriptor_set_layouts, pipeline_layout, gfx_pipeline))
}

impl HalState {
    pub fn new(window: &Window) -> Result<Self, &'static str> {
        // Create An Instance
        let instance = back::Instance::create(WINDOW_NAME, 1);

        // Create A Surface
        let mut surface = instance.create_surface(window);

        // Select An Adapter
        let adapter = instance
            .enumerate_adapters()
            .into_iter()
            .find(|a| {
                a.queue_families
                    .iter()
                    .any(|qf| qf.supports_graphics() && surface.supports_queue_family(qf))
            })
            .ok_or("Couldn't find a graphical Adapter!")?;

        // Open A Device and take out a QueueGroup
        let (mut device, queue_group) = {
            let (device, queue_group) = adapter
                .open_with::<_, hal::Graphics>(1, |queue_family| {
                    surface.supports_queue_family(queue_family)
                })
                .unwrap();

            if !queue_group.queues.is_empty() {
                Ok(())
            } else {
                Err("The QueueGroup did not have any CommandQueues available!")
            }?;
            (device, queue_group)
        };

        // Create A Swapchain, this is extra long
        let (swapchain, extent, backbuffer, format, frames_in_flight) =
            HalState::build_swapchain(&mut surface, &adapter, &window, &device)?;

        // Create Our Sync Primitives
        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) = {
            let mut image_available_semaphores: Vec<<back::Backend as Backend>::Semaphore> = vec![];
            let mut render_finished_semaphores: Vec<<back::Backend as Backend>::Semaphore> = vec![];
            let mut in_flight_fences: Vec<<back::Backend as Backend>::Fence> = vec![];
            for _ in 0..frames_in_flight {
                in_flight_fences.push(
                    device
                        .create_fence(true)
                        .map_err(|_| "Could not create a fence!")?,
                );
                image_available_semaphores.push(
                    device
                        .create_semaphore()
                        .map_err(|_| "Could not create a semaphore!")?,
                );
                render_finished_semaphores.push(
                    device
                        .create_semaphore()
                        .map_err(|_| "Could not create a semaphore!")?,
                );
            }
            (
                image_available_semaphores,
                render_finished_semaphores,
                in_flight_fences,
            )
        };

        // Define A RenderPass
        let render_pass = {
            let color_attachment = Attachment {
                format: Some(format),
                samples: 1,
                ops: AttachmentOps {
                    load: AttachmentLoadOp::Clear,
                    store: AttachmentStoreOp::Store,
                },
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::Present,
            };
            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };
            unsafe {
                device
                    .create_render_pass(&[color_attachment], &[subpass], &[])
                    .map_err(|_| "Couldn't create a render pass!")?
            }
        };

        // Create The ImageViews
        let image_views: Vec<_> = match backbuffer {
            Backbuffer::Images(images) => images
                .into_iter()
                .map(|image| unsafe {
                    device
                        .create_image_view(
                            &image,
                            ViewKind::D2,
                            format,
                            Swizzle::NO,
                            SubresourceRange {
                                aspects: Aspects::COLOR,
                                levels: 0..1,
                                layers: 0..1,
                            },
                        )
                        .map_err(|_| "Couldn't create the image_view for the image!")
                })
                .collect::<Result<Vec<_>, &str>>()?,
            Backbuffer::Framebuffer(_) => unimplemented!("Can't handle framebuffer backbuffer!"),
        };

        // Create Our FrameBuffers
        let framebuffers: Vec<<back::Backend as Backend>::Framebuffer> = {
            image_views
                .iter()
                .map(|image_view| unsafe {
                    device
                        .create_framebuffer(
                            &render_pass,
                            vec![image_view],
                            Extent {
                                width: extent.width as u32,
                                height: extent.height as u32,
                                depth: 1,
                            },
                        )
                        .map_err(|_| "Failed to create a framebuffer!")
                })
                .collect::<Result<Vec<_>, &str>>()?
        };

        // Create Our CommandPool
        let mut command_pool = unsafe {
            device
                .create_command_pool_typed(&queue_group, CommandPoolCreateFlags::RESET_INDIVIDUAL)
                .map_err(|_| "Could not create the raw command pool!")?
        };

        // Create Our CommandBuffers
        let command_buffers: Vec<_> = framebuffers
            .iter()
            .map(|_| command_pool.acquire_command_buffer())
            .collect();

        // Build our pipeline and vertex buffer
        let (descriptor_set_layouts, pipeline_layout, graphics_pipeline) =
            create_pipeline(&mut device, extent, &render_pass)?;
        let (buffer, memory, requirements) = unsafe {
            const F32_XY_TRIANGLE: u64 = (size_of::<f32>() * 2 * 3) as u64;
            let mut buffer = device
                .create_buffer(F32_XY_TRIANGLE, BufferUsage::VERTEX)
                .map_err(|_| "Couldn't create a buffer for the vertices")?;
            let requirements = device.get_buffer_requirements(&buffer);
            let memory_type_id = adapter
                .physical_device
                .memory_properties()
                .memory_types
                .iter()
                .enumerate()
                .find(|&(id, memory_type)| {
                    requirements.type_mask & (1 << id) != 0
                        && memory_type.properties.contains(Properties::CPU_VISIBLE)
                })
                .map(|(id, _)| MemoryTypeId(id))
                .ok_or("Couldn't find a memory type to support the vertex buffer!")?;
            let memory = device
                .allocate_memory(memory_type_id, requirements.size)
                .map_err(|_| "Couldn't allocate vertex buffer memory")?;
            device
                .bind_buffer_memory(&memory, 0, &mut buffer)
                .map_err(|_| "Couldn't bind the buffer memory!")?;
            (buffer, memory, requirements)
        };

        Ok(Self {
            requirements,
            buffer: ManuallyDrop::new(buffer),
            memory: ManuallyDrop::new(memory),
            _instance: ManuallyDrop::new(instance),
            surface: surface,
            adapter: adapter,
            device: ManuallyDrop::new(device),
            queue_group,
            swapchain: ManuallyDrop::new(swapchain),
            render_area: extent.to_extent().rect(),
            render_pass: ManuallyDrop::new(render_pass),
            image_views,
            framebuffers,
            command_pool: ManuallyDrop::new(command_pool),
            command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            frames_in_flight,
            current_frame: 0,
            descriptor_set_layouts,
            pipeline_layout: ManuallyDrop::new(pipeline_layout),
            graphics_pipeline: ManuallyDrop::new(graphics_pipeline),
            should_recreate_swapchain: false,
            format: format,
        })
    }

    pub unsafe fn cleanup_framebuffers(&mut self) {
        for framebuffer in self.framebuffers.drain(..) {
            self.device.destroy_framebuffer(framebuffer);
        }
        for rtv in self.image_views.drain(..) {
            self.device.destroy_image_view(rtv);
        }
    }

    pub fn recreate_swapchain(&mut self) {
        // vkDeviceWaitIdle(device);

        // cleanupSwapChain();

        // createSwapChain();
        // createImageViews();
        // createRenderPass();
        // createGraphicsPipeline();
        // createFramebuffers();
        // createCommandBuffers();
    }

    fn build_swapchain(
        mut surface: &mut <back::Backend as Backend>::Surface,
        adapter: &Adapter<back::Backend>,
        window: &Window,
        device: &back::Device,
    ) -> Result<
        (
            <back::Backend as Backend>::Swapchain,
            Extent2D,
            Backbuffer<back::Backend>,
            Format,
            usize,
        ),
        &'static str,
    > {
        let (caps, preferred_formats, present_modes) =
            surface.compatibility(&adapter.physical_device);
        info!("{:?}", caps);
        info!("Preferred Formats: {:?}", preferred_formats);
        info!("Present Modes: {:?}", present_modes);
        //info!("Composite Alphas: {:?}", composite_alphas);
        //
        let present_mode = {
            use gfx_hal::window::PresentMode::*;
            [Mailbox, Fifo, Relaxed, Immediate]
                .iter()
                .cloned()
                .find(|pm| present_modes.contains(pm))
                .ok_or("No PresentMode values specified!")?
        };

        // XXX TODO figure out composite alpha
        // let composite_alpha = {
        //     use hal::CompositeAlpha;
        //     [CompositeAlpha::Opaque, CompositeAlpha::Inherit, CompositeAlpha::PreMultiplied, CompositeAlpha::PostMultiplied]
        //         .iter()
        //         .cloned()
        //         .find(|ca| composite_alphas.contains(ca))
        //         .ok_or("No CompositeAlpha values specified!")?
        // };
        let format = match preferred_formats {
            None => Format::Rgba8Srgb,
            Some(formats) => match formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .cloned()
            {
                Some(srgb_format) => srgb_format,
                None => formats
                    .get(0)
                    .cloned()
                    .ok_or("Preferred format list was empty!")?,
            },
        };
        let extent = {
            let window_client_area = window
                .get_inner_size()
                .ok_or("Window doesn't exist!")?
                .to_physical(window.get_hidpi_factor());
            Extent2D {
                width: caps.extents.end.width.min(window_client_area.width as u32),
                height: caps
                    .extents
                    .end
                    .height
                    .min(window_client_area.height as u32),
            }
        };
        let image_count = if present_mode == PresentMode::Mailbox {
            (caps.image_count.end - 1).min(3)
        } else {
            (caps.image_count.end - 1).min(2)
        };
        debug!("Present mode: {:#?}", present_mode);
        let swapchain_config = SwapchainConfig::from_caps(&caps, format, extent);
        // let swapchain_config = SwapchainConfig {
        //     present_mode,
        //     //composite_alpha,
        //     format,
        //     extent,
        //     image_count,
        //     image_layers,
        //     image_usage,
        // };
        info!("{:?}", swapchain_config);
        //
        let (swapchain, backbuffer) = unsafe {
            device
                .create_swapchain(&mut surface, swapchain_config, None)
                .map_err(|_| "Failed to create the swapchain!")?
        };
        Ok((swapchain, extent, backbuffer, format, image_count as usize))
    }

    pub fn draw_triangle_frame(&mut self, triangle: Triangle) -> Result<(), &'static str> {
        let tid = thread::current().id();
        debug!("thread id (draw_triangle_frame): {:#?}", tid);
        // SETUP FOR THIS FRAME
        let image_available = &self.image_available_semaphores[self.current_frame];
        let render_finished = &self.render_finished_semaphores[self.current_frame];
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

        let (i_u32, i_usize) = unsafe {
            let image_index = self
                .swapchain
                // .acquire_image(core::u64::MAX, FrameSync::Semaphore(image_available))
                .acquire_image(!0, Some(image_available), None)
                .map_err(|_| "Couldn't acquire an image from the swapchain!")?;
            (image_index, image_index as usize)
        };

        let flight_fence = &self.in_flight_fences[i_usize];
        unsafe {
            self.device
                .wait_for_fence(flight_fence, core::u64::MAX)
                .map_err(|_| "Failed to wait on the fence!")?;
            self.device
                .reset_fence(flight_fence)
                .map_err(|_| "Couldn't reset the fence!")?;
        }

        // WRITE THE TRIANGLE DATA
        unsafe {
            let mut data_target = self
                .device
                .acquire_mapping_writer(&self.memory, 0..self.requirements.size)
                .map_err(|_| "Failed to acquire a memory writer!")?;
            let points = triangle.points_flat();
            data_target[..points.len()].copy_from_slice(&points);
            self.device
                .release_mapping_writer(data_target)
                .map_err(|_| "Couldn't release the mapping writer!")?;
        }

        // RECORD COMMANDS
        unsafe {
            let buffer = &mut self.command_buffers[i_usize];
            const TRIANGLE_CLEAR: [ClearValue; 1] =
                [ClearValue::Color(ClearColor::Float([0.1, 0.2, 0.3, 1.0]))];
            buffer.begin(false);
            {
                let mut encoder = buffer.begin_render_pass_inline(
                    &self.render_pass,
                    &self.framebuffers[i_usize],
                    self.render_area,
                    TRIANGLE_CLEAR.iter(),
                );
                encoder.bind_graphics_pipeline(&self.graphics_pipeline);
                // Here we must force the Deref impl of ManuallyDrop to play nice.
                let buffer_ref: &<back::Backend as Backend>::Buffer = &self.buffer;
                let buffers: ArrayVec<[_; 1]> = [(buffer_ref, 0)].into();
                encoder.bind_vertex_buffers(0, buffers);
                encoder.draw(0..3, 0..1);
            }
            buffer.finish();
        }

        // SUBMISSION AND PRESENT
        let command_buffers = &self.command_buffers[i_usize..=i_usize];
        let wait_semaphores: ArrayVec<[_; 1]> =
            [(image_available, PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
        let signal_semaphores: ArrayVec<[_; 1]> = [render_finished].into();
        // yes, you have to write it twice like this. yes, it's silly.
        let present_wait_semaphores: ArrayVec<[_; 1]> = [render_finished].into();
        let submission = Submission {
            command_buffers,
            wait_semaphores,
            signal_semaphores,
        };
        let the_command_queue = &mut self.queue_group.queues[0];
        unsafe {
            // The list of work in the CommandBuffer is sent to the GPU and processed.
            the_command_queue.submit(submission, Some(flight_fence));

            // This is where something actually gets drawn! The swapchain waits for the
            // work submitted to the GPU to finish processing, and then displays the results.
            self.swapchain
                .present(the_command_queue, i_u32, present_wait_semaphores)
                .map_err(|e| {
                    debug!("Failed to present into the swapchain: {:#?}", e);
                    "Failed to present into the swapchain!"
                })
        }
    }
}

#[derive(Debug, Clone)]
pub enum UserInput {
    Noop,
    EndRequested,
    NewFrameSize(Option<(f64, f64)>),
    NewMousePosition(Option<(f64, f64)>),
    Keypress(u32),
    // pub keypress: winit::VirtualKeyCode,
}

impl UserInput {
    pub fn poll_events_loop(events_loop: &mut EventsLoop) -> Option<Self> {
        let mut output = UserInput::Noop;
        // XXX should maybe make default method instead?
        // let mut output = UserInput::default();
        events_loop.poll_events(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                output = UserInput::EndRequested;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(logical),
                ..
            } => {
                output = UserInput::NewFrameSize(Some((logical.width, logical.height)));
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                output = UserInput::NewMousePosition(Some((position.x, position.y)));
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                    ..
                } => match (virtual_code, state) {
                    (winit::VirtualKeyCode::Escape, _) => {
                        output = UserInput::EndRequested;
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        });
        match output {
            UserInput::Noop => None,
            r => Some(r),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocalState {
    pub frame_width: f64,
    pub frame_height: f64,
    pub mouse_x: f64,
    pub mouse_y: f64,
}

impl LocalState {
    pub fn update_from_input(&mut self, input: UserInput) {
        debug!("update_from_input");
        match input {
            UserInput::NewFrameSize(Some(frame_size)) => {
                self.frame_width = frame_size.0;
                self.frame_height = frame_size.1;
            }
            UserInput::NewMousePosition(Some(position)) => {
                self.mouse_x = position.0;
                self.mouse_y = position.1;
            }
            _ => (),
        };
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub points: [[f32; 2]; 3],
}
impl Triangle {
    pub fn points_flat(self) -> [f32; 6] {
        let [[a, b], [c, d], [e, f]] = self.points;
        [a, b, c, d, e, f]
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

pub trait GraphicalGame {
    /// Initialize a new instance of the application. Will initialize the
    /// window and graphics state, and then return a new T: <GraphicalGame>.
    fn init() -> Self;

    /// Gets the compiled shader code from the SHADER_DIR
    fn get_shader_code(shader_name: &str) -> Result<ShaderData, Box<dyn Error>> {
        // I will probably want to use some human-readable JSON config for top-level
        // map configurations.
        let shader_path = Path::new(SHADER_DIR).join(shader_name);
        let mut shader_file = File::open(shader_path)?;
        let mut shader_data = Vec::<u8>::new();
        shader_file.read_to_end(&mut shader_data)?;

        Ok(shader_data)
    }

    fn do_the_render(&mut self, local_state: &LocalState) -> Result<(), &'static str>;

    fn run(self);

    fn main_loop(self);
}

type ShaderData = Vec<u8>;
