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
use std::cell::{RefCell, RefMut};
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::graphics::hal::Capability;
use arrayvec::ArrayVec;
use core::mem::{size_of, ManuallyDrop};
use hal::{
    adapter::{Adapter, MemoryTypeId, PhysicalDevice},
    buffer::Usage as BufferUsage,
    command::{self, ClearColor, ClearValue, CommandBuffer, MultiShot, Primary},
    device::Device,
    format::{self, Aspects, ChannelType, Format, Swizzle},
    image::{Extent, Layout, SubresourceRange, Usage, ViewKind},
    memory::{Properties, Requirements},
    pass::{
        Attachment, AttachmentLoadOp, AttachmentOps, AttachmentRef, AttachmentStoreOp, Subpass,
        SubpassDesc,
    },
    pool::{self, CommandPool, CommandPoolCreateFlags},
    pso::{
        self, AttributeDesc, BakedStates, BasePipeline, BlendDesc, BlendOp, BlendState,
        ColorBlendDesc, ColorMask, DepthStencilDesc, DepthTest, DescriptorSetLayoutBinding,
        Element, EntryPoint, Face, Factor, FrontFace, GraphicsPipelineDesc, GraphicsShaderSet,
        InputAssemblerDesc, LogicOp, PipelineCreationFlags, PipelineStage, PolygonMode, Rasterizer,
        Rect, ShaderStageFlags, Specialization, StencilTest, VertexBufferDesc, Viewport,
    },
    queue::{self, family::QueueGroup, Submission},
    window::{self, Backbuffer, Extent2D, FrameSync, PresentMode, Swapchain, SwapchainConfig},
    Backend, Gpu, Graphics, Instance, Primitive, QueueFamily, Surface,
};
use winit::{dpi, ControlFlow, Event, EventsLoop, Window, WindowBuilder, WindowEvent};

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
    drop(application);
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
    _adapter: Adapter<back::Backend>,
    _surface: <back::Backend as Backend>::Surface,
    _instance: ManuallyDrop<back::Instance>,
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
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self._instance);
        }
    }
}

impl HalState {
    /// Clean up things on the graphics device's state, right now the
    /// render pipeline & layout need to be destroyed, the swapchain needs to
    /// be destroyed, and the frame images need to be destroyed.
    unsafe fn clean_up(self) {}

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
            let queue_family = adapter
                .queue_families
                .iter()
                .find(|qf| qf.supports_graphics() && surface.supports_queue_family(qf))
                .ok_or("Couldn't find a QueueFamily with graphics!")?;
            let Gpu { device, mut queues } = unsafe {
                adapter
                    .physical_device
                    .open(&[(&queue_family, &[1.0; 1])])
                    .map_err(|_| "Couldn't open the PhysicalDevice!")?
            };
            let queue_group = queues
                .take::<Graphics>(queue_family.id())
                .ok_or("Couldn't take ownership of the QueueGroup!")?;
            if queue_group.queues.len() > 0 {
                Ok(())
            } else {
                Err("The QueueGroup did not have any CommandQueues available!")
            }?;
            (device, queue_group)
        };

        // Create A Swapchain, this is extra long
        let (swapchain, extent, backbuffer, format, frames_in_flight) = {
            let (caps, preferred_formats, present_modes, composite_alphas) =
                surface.compatibility(&adapter.physical_device);
            info!("{:?}", caps);
            info!("Preferred Formats: {:?}", preferred_formats);
            info!("Present Modes: {:?}", present_modes);
            info!("Composite Alphas: {:?}", composite_alphas);
            //
            let present_mode = {
                use gfx_hal::window::PresentMode::*;
                [Mailbox, Fifo, Relaxed, Immediate]
                    .iter()
                    .cloned()
                    .find(|pm| present_modes.contains(pm))
                    .ok_or("No PresentMode values specified!")?
            };
            let composite_alpha = {
                use gfx_hal::window::CompositeAlpha::*;
                [Opaque, Inherit, PreMultiplied, PostMultiplied]
                    .iter()
                    .cloned()
                    .find(|ca| composite_alphas.contains(ca))
                    .ok_or("No CompositeAlpha values specified!")?
            };
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
            let image_layers = 1;
            let image_usage = if caps.usage.contains(Usage::COLOR_ATTACHMENT) {
                Usage::COLOR_ATTACHMENT
            } else {
                Err("The Surface isn't capable of supporting color!")?
            };
            let swapchain_config = SwapchainConfig {
                present_mode,
                composite_alpha,
                format,
                extent,
                image_count,
                image_layers,
                image_usage,
            };
            info!("{:?}", swapchain_config);
            //
            let (swapchain, backbuffer) = unsafe {
                device
                    .create_swapchain(&mut surface, swapchain_config, None)
                    .map_err(|_| "Failed to create the swapchain!")?
            };
            (swapchain, extent, backbuffer, format, image_count as usize)
        };

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
            RustTowerDefenseApplication::create_pipeline(&mut device, extent, &render_pass)?;
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
            _surface: surface,
            _adapter: adapter,
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
        })
    }

    pub fn draw_triangle_frame(&mut self, triangle: Triangle) -> Result<(), &'static str> {
        // SETUP FOR THIS FRAME
        let image_available = &self.image_available_semaphores[self.current_frame];
        let render_finished = &self.render_finished_semaphores[self.current_frame];
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

        let (i_u32, i_usize) = unsafe {
            let image_index = self
                .swapchain
                .acquire_image(core::u64::MAX, FrameSync::Semaphore(image_available))
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
            the_command_queue.submit(submission, Some(flight_fence));
            self.swapchain
                .present(the_command_queue, i_u32, present_wait_semaphores)
                .map_err(|_| "Failed to present into the swapchain!")
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UserInput {
    pub end_requested: bool,
    pub new_frame_size: Option<(f64, f64)>,
    pub new_mouse_position: Option<(f64, f64)>,
}

impl UserInput {
    pub fn poll_events_loop(events_loop: &mut EventsLoop) -> Self {
        let mut output = UserInput::default();
        events_loop.poll_events(|event| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => output.end_requested = true,
            Event::WindowEvent {
                event: WindowEvent::Resized(logical),
                ..
            } => {
                output.new_frame_size = Some((logical.width, logical.height));
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                output.new_mouse_position = Some((position.x, position.y));
            }
            _ => (),
        });
        output
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
        if let Some(frame_size) = input.new_frame_size {
            self.frame_width = frame_size.0;
            self.frame_height = frame_size.1;
        }
        if let Some(position) = input.new_mouse_position {
            self.mouse_x = position.0;
            self.mouse_y = position.1;
        }
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

/// The application contains the window and graphics
/// drivers' states. This is the main interface to interact
/// with the application. We would probably expand this to be
/// the systems' manager at some point, or reference it from
/// the systems' manager.
struct RustTowerDefenseApplication {
    hal_state: RefCell<HalState>,
    window_state: WindowState,
}

type ShaderData = Vec<u8>;

impl RustTowerDefenseApplication {
    /// Initialize a new instance of the application. Will initialize the
    /// window and graphics state, and then return a new RustTowerDefenseApplication.
    pub fn init() -> RustTowerDefenseApplication {
        let window_state = RustTowerDefenseApplication::init_window();
        let mut hal_state = match HalState::new(&window_state.window) {
            Ok(state) => state,
            Err(e) => panic!(e),
        };

        RustTowerDefenseApplication {
            hal_state: RefCell::new(hal_state),
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
                Some(RustTowerDefenseApplication::prompt_for_monitor(
                    &events_loop,
                ))
            } else {
                None
            }
        }

        #[cfg(not(target_os = "macos"))]
        Some(RustTowerDefenseApplication::prompt_for_monitor(
            &events_loop,
        ))
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
                .open(&families)
                .expect("Could not create device.")
        };

        let mut queue_group = queues
            .take::<Graphics>(family.id())
            .expect("Could not take ownership of relevant queue group.");

        let command_queues: Vec<_> = queue_group.queues.drain(..1).collect();

        (device, command_queues, family.queue_type(), family.id())
    }

    fn init_hal(window: &Window) -> Result<HalState, &'static str> {
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
            let queue_family = adapter
                .queue_families
                .iter()
                .find(|qf| qf.supports_graphics() && surface.supports_queue_family(qf))
                .ok_or("Couldn't find a QueueFamily with graphics!")?;
            let Gpu { device, mut queues } = unsafe {
                adapter
                    .physical_device
                    .open(&[(&queue_family, &[1.0; 1])])
                    .map_err(|_| "Couldn't open the PhysicalDevice!")?
            };
            let queue_group = queues
                .take::<Graphics>(queue_family.id())
                .ok_or("Couldn't take ownership of the QueueGroup!")?;
            if queue_group.queues.len() > 0 {
                Ok(())
            } else {
                Err("The QueueGroup did not have any CommandQueues available!")
            }?;
            (device, queue_group)
        };

        // Create A Swapchain, this is extra long
        let (swapchain, extent, backbuffer, format, frames_in_flight) = {
            let (caps, preferred_formats, present_modes, composite_alphas) =
                surface.compatibility(&adapter.physical_device);
            info!("{:?}", caps);
            info!("Preferred Formats: {:?}", preferred_formats);
            info!("Present Modes: {:?}", present_modes);
            info!("Composite Alphas: {:?}", composite_alphas);
            //
            let present_mode = {
                use gfx_hal::window::PresentMode::*;
                [Mailbox, Fifo, Relaxed, Immediate]
                    .iter()
                    .cloned()
                    .find(|pm| present_modes.contains(pm))
                    .ok_or("No PresentMode values specified!")?
            };
            let composite_alpha = {
                use gfx_hal::window::CompositeAlpha::*;
                [Opaque, Inherit, PreMultiplied, PostMultiplied]
                    .iter()
                    .cloned()
                    .find(|ca| composite_alphas.contains(ca))
                    .ok_or("No CompositeAlpha values specified!")?
            };
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
            let image_layers = 1;
            let image_usage = if caps.usage.contains(Usage::COLOR_ATTACHMENT) {
                Usage::COLOR_ATTACHMENT
            } else {
                Err("The Surface isn't capable of supporting color!")?
            };
            let swapchain_config = SwapchainConfig {
                present_mode,
                composite_alpha,
                format,
                extent,
                image_count,
                image_layers,
                image_usage,
            };
            info!("{:?}", swapchain_config);
            //
            let (swapchain, backbuffer) = unsafe {
                device
                    .create_swapchain(&mut surface, swapchain_config, None)
                    .map_err(|_| "Failed to create the swapchain!")?
            };
            (swapchain, extent, backbuffer, format, image_count as usize)
        };

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
            RustTowerDefenseApplication::create_pipeline(&mut device, extent, &render_pass)?;
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

        Ok(HalState {
            requirements,
            buffer: ManuallyDrop::new(buffer),
            memory: ManuallyDrop::new(memory),
            _instance: ManuallyDrop::new(instance),
            _surface: surface,
            _adapter: adapter,
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
        })
    }
    // /// Creates a new instance of the graphics device's state. This maintains
    // /// everything necessary to set up the rendering pipeline from graphics device
    // /// to window surface.
    // unsafe fn init_hal(window: &Window) -> HalState {
    //     let instance = RustTowerDefenseApplication::create_device_instance();
    //     let mut adapter = RustTowerDefenseApplication::pick_adapter(&instance);
    //     let mut surface = RustTowerDefenseApplication::create_surface(&instance, window);
    //     let (device, command_queues, queue_type, qf_id) =
    //         RustTowerDefenseApplication::create_device_with_graphics_queues(&mut adapter, &surface);
    //     let (swapchain, extent, backbuffer, format) =
    //         RustTowerDefenseApplication::create_swap_chain(&adapter, &device, &mut surface, None);
    //     let frame_images =
    //         RustTowerDefenseApplication::create_image_views(backbuffer, format, &device);
    //     let render_pass = RustTowerDefenseApplication::create_render_pass(&device, Some(format));
    //     let (descriptor_set_layouts, pipeline_layout, graphics_pipeline) =
    //         RustTowerDefenseApplication::create_graphics_pipeline(&device, extent, &render_pass);
    //     let framebuffers = RustTowerDefenseApplication::create_framebuffers(
    //         &device,
    //         &render_pass,
    //         &frame_images,
    //         extent,
    //     );
    //     let mut command_pool =
    //         RustTowerDefenseApplication::create_command_pool(&device, queue_type, qf_id);
    //     let command_buffers = RustTowerDefenseApplication::create_command_buffers(
    //         &mut command_pool,
    //         &render_pass,
    //         &framebuffers,
    //         extent,
    //         &graphics_pipeline,
    //     );
    //     let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
    //         RustTowerDefenseApplication::create_sync_objects(&device);

    //     HalState {
    //         requirements,
    //         buffer: ManuallyDrop::new(buffer),
    //         memory: ManuallyDrop::new(memory),
    //         in_flight_fences,
    //         render_finished_semaphores,
    //         image_available_semaphores,
    //         command_buffers,
    //         command_pool,
    //         framebuffers,
    //         graphics_pipeline,
    //         descriptor_set_layouts,
    //         pipeline_layout,
    //         render_pass,
    //         frame_images,
    //         _format: format,
    //         swapchain,
    //         command_queues,
    //         device,
    //         _surface: surface,
    //         _adapter: adapter,
    //         _instance: instance,
    //     }
    // }

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
        let (caps, formats, _present_modes, _composite_alphas) =
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
                        ViewKind::D2,
                        format,
                        format::Swizzle::NO,
                        SubresourceRange {
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

        let ops = AttachmentOps {
            load: AttachmentLoadOp::Clear,
            store: AttachmentStoreOp::Store,
        };

        let stencil_ops = AttachmentOps::DONT_CARE;

        let layouts = Layout::Undefined..Layout::Present;

        // In our case we'll have just a single color buffer attachment represented by one of the images from the swap chain.
        let color_attachment = Attachment {
            format,
            samples,
            ops,
            stencil_ops,
            layouts,
        };

        let color_attachment_ref: AttachmentRef = (0, Layout::ColorAttachmentOptimal);

        // hal assumes pipeline bind point is GRAPHICS
        let subpass = SubpassDesc {
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

    // /// Creates the actual graphics pipeline, consisting of programmable (shader) elements as well
    // /// as fixed functions.
    // unsafe fn create_graphics_pipeline(
    //     device: &<back::Backend as Backend>::Device,
    //     extent: window::Extent2D,
    //     render_pass: &<back::Backend as Backend>::RenderPass,
    // ) -> (
    //     Vec<<back::Backend as Backend>::DescriptorSetLayout>,
    //     <back::Backend as Backend>::PipelineLayout,
    //     <back::Backend as Backend>::GraphicsPipeline,
    // ) {
    //     let vert_shader_code = RustTowerDefenseApplication::get_shader_code("test.vert.spv")
    //         .expect("Error loading vertex shader code.");

    //     let frag_shader_code = RustTowerDefenseApplication::get_shader_code("test.frag.spv")
    //         .expect("Error loading fragment shader code.");

    //     let vert_shader_module = device
    //         .create_shader_module(&vert_shader_code)
    //         .expect("Error creating shader module.");
    //     let frag_shader_module = device
    //         .create_shader_module(&frag_shader_code)
    //         .expect("Error creating fragment module.");

    //     let (ds_layouts, pipeline_layout, graphics_pipeline) = {
    //         let (vs_entry, fs_entry) = (
    //             pso::EntryPoint::<back::Backend> {
    //                 entry: "main",
    //                 module: &vert_shader_module,
    //                 specialization: hal::pso::Specialization {
    //                     constants: &[],
    //                     data: &[],
    //                 },
    //             },
    //             pso::EntryPoint::<back::Backend> {
    //                 entry: "main",
    //                 module: &frag_shader_module,
    //                 specialization: hal::pso::Specialization {
    //                     constants: &[],
    //                     data: &[],
    //                 },
    //             },
    //         );

    //         let shaders = pso::GraphicsShaderSet {
    //             vertex: vs_entry,
    //             hull: None,
    //             domain: None,
    //             geometry: None,
    //             fragment: Some(fs_entry),
    //         };

    //         let rasterizer = pso::Rasterizer {
    //             depth_clamping: false,
    //             polygon_mode: pso::PolygonMode::Fill,
    //             cull_face: <pso::Face>::BACK,
    //             front_face: pso::FrontFace::Clockwise,
    //             depth_bias: None,
    //             conservative: false,
    //         };

    //         // no need to set up vertex input format, as it is hardcoded
    //         let vertex_buffers: Vec<pso::VertexBufferDesc> = Vec::new();
    //         let attributes: Vec<pso::AttributeDesc> = Vec::new();

    //         let input_assembler = pso::InputAssemblerDesc::new(Primitive::TriangleList);

    //         // implements optional blending description provided in vulkan-tutorial
    //         // After a fragment shader has returned a color, it needs to be combined with
    //         // the color that is already in the framebuffer. This transformation is known as
    //         // color blending and there are two ways to do it:
    //         // Mix the old and new value to produce a final color
    //         // Combine the old and new value using a bitwise operation
    //         // Not really sure what we do here :)
    //         // I think it's hardcoded, and this differs from Vulkan to HAL
    //         let blender = {
    //             let blend_state = pso::BlendState::On {
    //                 color: pso::BlendOp::Add {
    //                     src: pso::Factor::One,
    //                     dst: pso::Factor::Zero,
    //                 },
    //                 alpha: pso::BlendOp::Add {
    //                     src: pso::Factor::One,
    //                     dst: pso::Factor::Zero,
    //                 },
    //             };

    //             pso::BlendDesc {
    //                 logic_op: Some(pso::LogicOp::Copy),
    //                 targets: vec![pso::ColorBlendDesc(pso::ColorMask::ALL, blend_state)],
    //             }
    //         };

    //         let depth_stencil = pso::DepthStencilDesc {
    //             depth: pso::DepthTest::Off,
    //             depth_bounds: false,
    //             stencil: pso::StencilTest::Off,
    //         };

    //         // No multisampling -- it could be used for antialiasing
    //         let multisampling: Option<pso::Multisampling> = None;

    //         // viewports and scissors
    //         let baked_states = pso::BakedStates {
    //             viewport: Some(pso::Viewport {
    //                 rect: pso::Rect {
    //                     x: 0,
    //                     y: 0,
    //                     w: extent.width as i16,
    //                     h: extent.height as i16,
    //                 },
    //                 depth: (0.0..1.0),
    //             }),
    //             scissor: Some(pso::Rect {
    //                 x: 0,
    //                 y: 0,
    //                 w: extent.width as i16,
    //                 h: extent.height as i16,
    //             }),
    //             blend_color: None,
    //             depth_bounds: None,
    //         };

    //         // with HAL, user only needs to specify whether overall state is static or dynamic

    //         // pipeline layout
    //         let bindings = Vec::<pso::DescriptorSetLayoutBinding>::new();
    //         let immutable_samplers = Vec::<<back::Backend as Backend>::Sampler>::new();
    //         let ds_layouts: Vec<<back::Backend as Backend>::DescriptorSetLayout> = vec![device
    //             .create_descriptor_set_layout(bindings, immutable_samplers)
    //             .unwrap()];
    //         let push_constants = Vec::<(pso::ShaderStageFlags, std::ops::Range<u32>)>::new();
    //         let layout = device
    //             .create_pipeline_layout(&ds_layouts, push_constants)
    //             .unwrap();

    //         let subpass = Subpass {
    //             index: 0,
    //             main_pass: render_pass,
    //         };

    //         let flags = pso::PipelineCreationFlags::empty();

    //         let parent = pso::BasePipeline::None;

    //         let graphics_pipeline = {
    //             let desc = pso::GraphicsPipelineDesc {
    //                 shaders,
    //                 rasterizer,
    //                 vertex_buffers,
    //                 attributes,
    //                 input_assembler,
    //                 blender,
    //                 depth_stencil,
    //                 multisampling,
    //                 baked_states,
    //                 layout: &layout,
    //                 subpass,
    //                 flags,
    //                 parent,
    //             };

    //             device
    //                 .create_graphics_pipeline(&desc, None)
    //                 .expect("failed to create graphics pipeline!")
    //         };

    //         (ds_layouts, layout, graphics_pipeline)
    //     };

    //     device.destroy_shader_module(vert_shader_module);
    //     device.destroy_shader_module(frag_shader_module);

    //     (ds_layouts, pipeline_layout, graphics_pipeline)
    // }

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
                rate: 0,
            }];
            let attributes: Vec<AttributeDesc> = vec![AttributeDesc {
                location: 0,
                binding: 0,
                element: Element {
                    format: Format::Rg32Float,
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
        let mut framebuffers: Vec<<back::Backend as Backend>::Framebuffer> = Vec::new();

        unsafe {
            for (_, image_view) in frame_images.iter() {
                framebuffers.push(
                    device
                        .create_framebuffer(
                            render_pass,
                            vec![image_view],
                            Extent {
                                width: extent.width as _,
                                height: extent.height as _,
                                depth: 1,
                            },
                        )
                        .expect("failed to create framebuffer!"),
                );
            }
        }

        framebuffers
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

        let mut command_buffers: Vec<
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
            command_buffers.push(command_buffer);
        }

        command_buffers
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
        command_buffers: &[command::CommandBuffer<
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
            command_buffers: &command_buffers[i..i + 1],
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
            .unwrap();
        //.expect("presentation failed!");
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

    fn do_the_render(&mut self, local_state: &LocalState) -> Result<(), &'static str> {
        let x = ((local_state.mouse_x / local_state.frame_width) * 2.0) - 1.0;
        let y = ((local_state.mouse_y / local_state.frame_height) * 2.0) - 1.0;
        let triangle = Triangle {
            points: [[-0.5, 0.5], [-0.5, -0.5], [x as f32, y as f32]],
        };
        self.hal_state.borrow_mut().draw_triangle_frame(triangle)
    }

    /// Runs window state's event loop until a CloseRequested event is received
    /// This should take a event pipe to write keyboard events to that can
    /// be processed by other systems.
    fn main_loop(&mut self) {
        let (frame_width, frame_height) = self
            .window_state
            .window
            .get_inner_size()
            .map(|logical| logical.into())
            .unwrap_or((0.0, 0.0));
        let mut local_state = LocalState {
            frame_width,
            frame_height,
            mouse_x: 0.0,
            mouse_y: 0.0,
        };

        let mut events_loop = self
            .window_state
            .events_loop
            .take()
            .expect("events_loop does not exist!");

        loop {
            let inputs = UserInput::poll_events_loop(&mut events_loop);
            if inputs.end_requested {
                break;
            }
            if let Err(e) = self.do_the_render(&local_state) {
                error!("Rendering Error: {:?}", e);
                debug!("Auto-restarting HalState...");
                drop(self.hal_state.borrow_mut());
                match HalState::new(&self.window_state.window) {
                    Ok(state) => {
                        self.hal_state = RefCell::new(state);
                    }
                    Err(e) => panic!(e),
                };
            }
        }
    }
    //     events_loop.run_forever(|event| match event {
    //         Event::WindowEvent {
    //             event: WindowEvent::CloseRequested,
    //             ..
    //         } => {
    //             self.hal_state
    //                 .device
    //                 .wait_idle()
    //                 .expect("Queues are not going idle!");
    //             ControlFlow::Break
    //         }
    //         _ => {
    //             unsafe {
    //                 RustTowerDefenseApplication::draw_frame(
    //                     &self.hal_state.device,
    //                     &mut self.hal_state.command_queues,
    //                     &mut self.hal_state.swapchain,
    //                     &self.hal_state.command_buffers,
    //                     &self.hal_state.image_available_semaphores[current_frame],
    //                     &self.hal_state.render_finished_semaphores[current_frame],
    //                     &self.hal_state.in_flight_fences[current_frame],
    //                 );
    //             }

    //             current_frame = (current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    //             ;
    //             ControlFlow::Continue
    //         }
    //     });
    //     self.window_state.events_loop = Some(events_loop);
    // }
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
    //                     &self.hal_state.command_buffers,
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
}
