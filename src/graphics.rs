pub mod models;

extern crate cgmath;
extern crate time;
extern crate winit;

use vulkano::buffer::cpu_pool::CpuBufferPool;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::device::{Device, DeviceExtensions};
use vulkano::format::Format;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass};
use vulkano::image::attachment::AttachmentImage;
use vulkano::image::SwapchainImage;
use vulkano::instance::Instance;
use vulkano::instance::PhysicalDevice;
use vulkano::pipeline::vertex::TwoBuffersDefinition;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, GraphicsPipelineAbstract};
use vulkano::swapchain;
use vulkano::swapchain::{
    AcquireError, PresentMode, SurfaceTransform, Swapchain, SwapchainCreationError,
};
use vulkano::sync;
use vulkano::sync::GpuFuture;

use vulkano_win::VkSurfaceBuild;

use winit::Window;

use cgmath::{Matrix3, Matrix4, Point3, Rad, Vector3};

use std::iter;
use std::sync::Arc;
use std::time::Instant;

use models::source_engine;

#[derive(Copy, Clone)]
pub struct Vertex {
    position: (f32, f32, f32),
}

impl_vertex!(Vertex, position);

pub const VERTICES: [Vertex; 11] = [
    Vertex {
        position: (0.0, 0.0, 0.0),
    }, // dummy vector because in the original model indices
    // start at 1
    Vertex {
        position: (40.6266, 28.3457, -1.10804),
    },
    Vertex {
        position: (40.0714, 30.4443, -1.10804),
    },
    Vertex {
        position: (40.7155, 31.1438, -1.10804),
    },
    Vertex {
        position: (42.0257, 30.4443, -1.10804),
    },
    Vertex {
        position: (43.4692, 28.3457, -1.10804),
    },
    Vertex {
        position: (37.5425, 28.3457, 14.5117),
    },
    Vertex {
        position: (37.0303, 30.4443, 14.2938),
    },
    Vertex {
        position: (37.6244, 31.1438, 14.5466),
    },
    Vertex {
        position: (38.8331, 30.4443, 15.0609),
    },
    Vertex {
        position: (40.1647, 28.3457, 15.6274),
    },
];

#[derive(Copy, Clone)]
pub struct Normal {
    normal: (f32, f32, f32),
}

impl_vertex!(Normal, normal);

pub const NORMALS: [Normal; 11] = [
    Normal {
        normal: (0.0, 0.0, 0.0),
    }, // dummy vector because in the original model indices
    // start at 1
    Normal {
        normal: (-0.966742, -0.255752, 0.0),
    },
    Normal {
        normal: (-0.966824, 0.255443, 0.0),
    },
    Normal {
        normal: (-0.092052, 0.995754, 0.0),
    },
    Normal {
        normal: (0.68205, 0.731305, 0.0),
    },
    Normal {
        normal: (0.870301, 0.492521, -0.0),
    },
    Normal {
        normal: (-0.893014, -0.256345, -0.369882),
    },
    Normal {
        normal: (-0.893437, 0.255997, -0.369102),
    },
    Normal {
        normal: (-0.0838771, 0.995843, -0.0355068),
    },
    Normal {
        normal: (0.629724, 0.73186, 0.260439),
    },
    Normal {
        normal: (0.803725, 0.49337, 0.332584),
    },
];

pub const INDICES: [u16; 642] = [
    7, 6, 1, 1, 2, 7, 8, 7, 2, 2, 3, 8, 9, 8, 3, 3, 4, 9, 10, 9, 4, 4, 5, 10, 12, 11, 6, 6, 7, 12,
    13, 12, 7, 7, 8, 13, 14, 13, 8, 8, 9, 14, 15, 14, 9, 9, 10, 15, 17, 16, 11, 11, 12, 17, 18, 17,
    12, 12, 13, 18, 19, 18, 13, 13, 14, 19, 20, 19, 14, 14, 15, 20, 22, 21, 16, 16, 17, 22, 23, 22,
    17, 17, 18, 23, 24, 23, 18, 18, 19, 24, 25, 24, 19, 19, 20, 25, 27, 26, 21, 21, 22, 27, 28, 27,
    22, 22, 23, 28, 29, 28, 23, 23, 24, 29, 30, 29, 24, 24, 25, 30, 32, 31, 26, 26, 27, 32, 33, 32,
    27, 27, 28, 33, 34, 33, 28, 28, 29, 34, 35, 34, 29, 29, 30, 35, 37, 36, 31, 31, 32, 37, 38, 37,
    32, 32, 33, 38, 39, 38, 33, 33, 34, 39, 40, 39, 34, 34, 35, 40, 42, 41, 36, 36, 37, 42, 43, 42,
    37, 37, 38, 43, 44, 43, 38, 38, 39, 44, 45, 44, 39, 39, 40, 45, 47, 46, 41, 41, 42, 47, 48, 47,
    42, 42, 43, 48, 49, 48, 43, 43, 44, 49, 50, 49, 44, 44, 45, 50, 52, 51, 46, 46, 47, 52, 53, 52,
    47, 47, 48, 53, 54, 53, 48, 48, 49, 54, 55, 54, 49, 49, 50, 55, 57, 56, 51, 51, 52, 57, 58, 57,
    52, 52, 53, 58, 59, 58, 53, 53, 54, 59, 60, 59, 54, 54, 55, 60, 62, 61, 56, 56, 57, 62, 63, 62,
    57, 57, 58, 63, 64, 63, 58, 58, 59, 64, 65, 64, 59, 59, 60, 65, 67, 66, 61, 61, 62, 67, 68, 67,
    62, 62, 63, 68, 69, 68, 63, 63, 64, 69, 70, 69, 64, 64, 65, 70, 72, 71, 66, 66, 67, 72, 73, 72,
    67, 67, 68, 73, 74, 73, 68, 68, 69, 74, 75, 74, 69, 69, 70, 75, 77, 76, 71, 71, 72, 77, 78, 77,
    72, 72, 73, 78, 79, 78, 73, 73, 74, 79, 80, 79, 74, 74, 75, 80, 2, 1, 76, 76, 77, 2, 3, 2, 77,
    77, 78, 3, 4, 3, 78, 78, 79, 4, 5, 4, 79, 79, 80, 5, 85, 10, 5, 5, 81, 85, 86, 85, 81, 81, 82,
    86, 87, 86, 82, 82, 83, 87, 88, 87, 83, 83, 84, 88, 89, 15, 10, 10, 85, 89, 90, 89, 85, 85, 86,
    90, 91, 90, 86, 86, 87, 91, 92, 91, 87, 87, 88, 92, 93, 20, 15, 15, 89, 93, 94, 93, 89, 89, 90,
    94, 95, 94, 90, 90, 91, 95, 96, 95, 91, 91, 92, 96, 97, 25, 20, 20, 93, 97, 98, 97, 93, 93, 94,
    98, 99, 98, 94, 94, 95, 99, 100, 99, 95, 95, 96, 100, 101, 30, 25, 25, 97, 101, 102, 101, 97,
    97, 98, 102, 103, 102, 98, 98, 99, 103, 104, 103, 99, 99, 100, 104, 105, 35, 30, 30, 101, 105,
    106, 105, 101, 101, 102, 106, 107, 106, 102, 102, 103, 107, 108, 107, 103, 103, 104, 108, 109,
    40, 35, 35, 105, 109, 110, 109, 105, 105, 106, 110, 111, 110, 106, 106, 107, 111, 112, 111,
    107, 107, 108, 112, 113, 45, 40, 40, 109, 113, 114, 113, 109, 109, 110, 114, 115, 114, 110,
    110, 111, 115, 116, 115, 111, 111, 112, 116, 117, 50, 45, 45, 113, 117, 118, 117, 113, 113,
    114, 118, 119, 118, 114, 114, 115, 119, 120, 119, 115, 115, 116, 120, 121, 55, 50, 50, 117,
    121, 122, 121, 117, 117, 118, 122, 123, 122, 118, 118, 119, 123, 124, 123, 119, 119, 120, 124,
    125, 60, 55, 55, 121, 125, 126, 125, 121, 121, 122, 126, 127, 126, 122, 122, 123, 127,
];

pub fn run() {
    let model = source_engine::read_source_engine_model("player/ctm_sas_variantA").unwrap();
    info!("Model id {}", model.mdl_file.header.id);

    let vertices: Vec<Vertex> = model.vertices;
    let normals: Vec<Vertex> = model.normals;

    let extensions = vulkano_win::required_extensions();
    let instance = Instance::new(None, &extensions, None).unwrap();

    let physical = PhysicalDevice::enumerate(&instance).next().unwrap();
    info!(
        "Using device: {} (type: {:?})",
        physical.name(),
        physical.ty()
    );

    let mut events_loop = winit::EventsLoop::new();
    let surface = winit::WindowBuilder::new()
        .build_vk_surface(&events_loop, instance.clone())
        .unwrap();
    let window = surface.window();

    // unlike the triangle example we need to keep track of the width and height so we can calculate
    // render the teapot with the correct aspect ratio.
    let mut dimensions = if let Some(dimensions) = window.get_inner_size() {
        let dimensions: (u32, u32) = dimensions.to_physical(window.get_hidpi_factor()).into();
        [dimensions.0, dimensions.1]
    } else {
        return;
    };

    let queue_family = physical
        .queue_families()
        .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
        .unwrap();

    let device_ext = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::none()
    };

    let (device, mut queues) = Device::new(
        physical,
        physical.supported_features(),
        &device_ext,
        [(queue_family, 0.5)].iter().cloned(),
    )
    .unwrap();

    let queue = queues.next().unwrap();

    let (mut swapchain, images) = {
        let caps = surface.capabilities(physical).unwrap();
        let usage = caps.supported_usage_flags;
        let format = caps.supported_formats[0].0;
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();

        Swapchain::new(
            device.clone(),
            surface.clone(),
            caps.min_image_count,
            format,
            dimensions,
            1,
            usage,
            &queue,
            SurfaceTransform::Identity,
            alpha,
            PresentMode::Fifo,
            true,
            None,
        )
        .unwrap()
    };

    // let vertices = VERTICES.iter().cloned();
    let vertices = vertices.iter().cloned();
    let vertex_buffer =
        CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), vertices).unwrap();

    // let normals = NORMALS.iter().cloned();
    let normals = normals.iter().cloned();
    let normals_buffer =
        CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), normals).unwrap();

    let indices = INDICES.iter().cloned();
    let index_buffer =
        CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), indices).unwrap();

    let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(device.clone(), BufferUsage::all());

    let vs = vs::Shader::load(device.clone()).unwrap();
    let fs = fs::Shader::load(device.clone()).unwrap();

    let render_pass = Arc::new(
        single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16Unorm,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        )
        .unwrap(),
    );

    let (mut pipeline, mut framebuffers) =
        window_size_dependent_setup(device.clone(), &vs, &fs, &images, render_pass.clone());
    let mut recreate_swapchain = false;

    let mut previous_frame = Box::new(sync::now(device.clone())) as Box<GpuFuture>;
    let rotation_start = Instant::now();

    loop {
        previous_frame.cleanup_finished();

        if recreate_swapchain {
            dimensions = if let Some(dimensions) = window.get_inner_size() {
                let dimensions: (u32, u32) =
                    dimensions.to_physical(window.get_hidpi_factor()).into();
                [dimensions.0, dimensions.1]
            } else {
                return;
            };

            let (new_swapchain, new_images) = match swapchain.recreate_with_dimension(dimensions) {
                Ok(r) => r,
                Err(SwapchainCreationError::UnsupportedDimensions) => continue,
                Err(err) => panic!("{:?}", err),
            };
            swapchain = new_swapchain;

            let (new_pipeline, new_framebuffers) = window_size_dependent_setup(
                device.clone(),
                &vs,
                &fs,
                &new_images,
                render_pass.clone(),
            );
            pipeline = new_pipeline;
            framebuffers = new_framebuffers;

            recreate_swapchain = false;
        }

        let uniform_buffer_subbuffer = {
            let elapsed = rotation_start.elapsed();
            let rotation =
                elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
            let rotation = Matrix3::from_angle_y(Rad(rotation as f32));

            // note: this teapot was meant for OpenGL where the origin is at the lower left
            //       instead the origin is at the upper left in Vulkan, so we reverse the Y axis
            let aspect_ratio = dimensions[0] as f32 / dimensions[1] as f32;
            let proj =
                cgmath::perspective(Rad(std::f32::consts::FRAC_PI_2), aspect_ratio, 0.01, 100.0);
            let view = Matrix4::look_at(
                Point3::new(0.3, 0.3, 1.0),
                Point3::new(0.0, 0.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
            );
            let scale = Matrix4::from_scale(0.01);

            let uniform_data = vs::ty::Data {
                world: Matrix4::from(rotation).into(),
                view: (view * scale).into(),
                proj: proj.into(),
            };

            uniform_buffer.next(uniform_data).unwrap()
        };

        let set = Arc::new(
            PersistentDescriptorSet::start(pipeline.clone(), 0)
                .add_buffer(uniform_buffer_subbuffer)
                .unwrap()
                .build()
                .unwrap(),
        );

        let (image_num, acquire_future) =
            match swapchain::acquire_next_image(swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    recreate_swapchain = true;
                    continue;
                }
                Err(err) => panic!("{:?}", err),
            };

        let command_buffer =
            AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family())
                .unwrap()
                .begin_render_pass(
                    framebuffers[image_num].clone(),
                    false,
                    vec![[0.0, 0.0, 1.0, 1.0].into(), 1f32.into()],
                )
                .unwrap()
                .draw(
                    pipeline.clone(),
                    &DynamicState::none(),
                    vec![vertex_buffer.clone(), normals_buffer.clone()],
                    set.clone(),
                    (),
                )
                .unwrap()
                .end_render_pass()
                .unwrap()
                .build()
                .unwrap();
        // let command_buffer =
        //     AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family())
        //         .unwrap()
        //         .begin_render_pass(
        //             framebuffers[image_num].clone(),
        //             false,
        //             vec![[0.0, 0.0, 1.0, 1.0].into(), 1f32.into()],
        //         )
        //         .unwrap()
        //         .draw_indexed(
        //             pipeline.clone(),
        //             &DynamicState::none(),
        //             vec![vertex_buffer.clone(), normals_buffer.clone()],
        //             index_buffer.clone(),
        //             set.clone(),
        //             (),
        //         )
        //         .unwrap()
        //         .end_render_pass()
        //         .unwrap()
        //         .build()
        //         .unwrap();

        let future = previous_frame
            .join(acquire_future)
            .then_execute(queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                previous_frame = Box::new(future) as Box<_>;
            }
            Err(sync::FlushError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame = Box::new(sync::now(device.clone())) as Box<_>;
            }
            Err(e) => {
                error!("{:?}", e);
                previous_frame = Box::new(sync::now(device.clone())) as Box<_>;
            }
        }

        let mut done = false;
        events_loop.poll_events(|ev| match ev {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => done = true,
            winit::Event::WindowEvent {
                event: winit::WindowEvent::Resized(_),
                ..
            } => recreate_swapchain = true,
            _ => (),
        });
        if done {
            return;
        }
    }
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    device: Arc<Device>,
    vs: &vs::Shader,
    fs: &fs::Shader,
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
) -> (
    Arc<GraphicsPipelineAbstract + Send + Sync>,
    Vec<Arc<FramebufferAbstract + Send + Sync>>,
) {
    let dimensions = images[0].dimensions();

    let depth_buffer =
        AttachmentImage::transient(device.clone(), dimensions, Format::D16Unorm).unwrap();

    let framebuffers = images
        .iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .add(depth_buffer.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>();

    // In the triangle example we use a dynamic viewport, as its a simple example.
    // However in the teapot example, we recreate the pipelines with a hardcoded viewport instead.
    // This allows the driver to optimize things, at the cost of slower window resizes.
    // https://computergraphics.stackexchange.com/questions/5742/vulkan-best-way-of-updating-pipeline-viewport
    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input(TwoBuffersDefinition::<Vertex, Normal>::new())
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .viewports(iter::once(Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0..1.0,
            }))
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap(),
    );

    (pipeline, framebuffers)
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "source_assets/shaders/teapot_vert.glsl"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "source_assets/shaders/teapot_frag.glsl"
    }
}
