mod ubo_example;
mod example;
mod model_example;

extern crate alloc;
extern crate nalgebra_glm as glm;
extern crate core;

use core::fmt::{Debug, Formatter};
use std::ffi::CString;
use std::mem::swap;
use std::time::Instant;
use ash::vk;

use simple_logger::SimpleLogger;

use tracing_subscriber::layer::SubscriberExt;
use winit;
use winit::window::{Window, WindowBuilder};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use imgui;
use imgui::BackendFlags;
use tracy_client::span_location;
use winit::error::EventLoopError;
use api_types::swapchain::SwapchainStatus;
use context::render_context::RenderContext;
use context::vulkan_render_context::{VulkanFrameObjects, VulkanRenderContext};
use framegraph::attachment::AttachmentReference;
use framegraph::frame::Frame;
use framegraph::frame_graph::FrameGraph;
use framegraph::pass_type::PassType;
use framegraph::pipeline::VulkanPipelineManager;
use framegraph::present_pass_node::PresentPassNode;
use framegraph::renderpass_manager::VulkanRenderpassManager;
use framegraph::vulkan_frame_graph::VulkanFrameGraph;
use passes::imgui_draw::ImguiRender;
use passes::clear;
use crate::example::Example;
use crate::model_example::ModelExample;
use crate::ubo_example::UboExample;

const MAX_FRAMES_IN_FLIGHT: u32 = 2;

struct Examples {
    examples: Vec<Box<dyn Example>>,
    active_example_index: Option<usize>
}

impl Examples {
    pub fn new(examples: Vec<Box<dyn Example>>) -> Self {
        Examples {
            examples,
            active_example_index: None
        }
    }
}

struct WindowedVulkanApp {
    window: Window,
    platform: WinitPlatform,
    imgui: imgui::Context,

    frame_index: u32,
    render_semaphores: Vec<vk::Semaphore>,
    frame_fences: Vec<vk::Fence>,
    frames: Vec<Option<Box<Frame>>>,

    // examples: Vec<Box<dyn Example>>,
    examples: Examples,

    imgui_renderer: ImguiRender,
    frame_graph: VulkanFrameGraph,

    render_context: VulkanRenderContext,

    tracy: tracy_client::Client
}

impl Debug for WindowedVulkanApp {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WindowedVulkanApp")
            .finish()
    }
}

impl WindowedVulkanApp {
    pub fn new(event_loop: &EventLoop<()>, title: &str, width: u32, height: u32) -> WindowedVulkanApp {
        let tracy = tracy_client::Client::start();

        // SimpleLogger::new().init().unwrap();
        simple_logger::init_with_level(log::Level::Warn).unwrap();

        let window = WindowBuilder::new()
            .with_title(title)
            // .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            // .with_disallow_hidpi(true)
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window");
        let scale_factor = window.scale_factor();

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        imgui.io_mut().backend_flags |= BackendFlags::RENDERER_HAS_VTX_OFFSET;

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        let mut render_context = {
            let c_title = CString::new(title).unwrap();
            let application_info = vk::ApplicationInfo::builder()
                .application_name(&c_title)
                .api_version(vk::API_VERSION_1_2);

            VulkanRenderContext::new(
                &application_info,
                true,
                8,
                Some(&window))
        };

        let frame_graph = VulkanFrameGraph::new(
            VulkanRenderpassManager::new(),
            VulkanPipelineManager::new());

        let max_frames_in_flight = {
            match render_context.get_swapchain() {
                Some(swapchain) => {
                    swapchain.get_images().len() as u32
                }
                None => {
                    MAX_FRAMES_IN_FLIGHT
                }
            }
        };

        let mut frame_fences: Vec<vk::Fence> = Vec::new();
        let mut render_semaphores: Vec<vk::Semaphore> = Vec::new();
        {
            // frame fences start as signaled so we don't wait the first time
            // we execute that frame
            let fence_create = vk::FenceCreateInfo::builder()
                .flags(vk::FenceCreateFlags::SIGNALED)
                .build();

            let semaphore_create = vk::SemaphoreCreateInfo::builder()
                .build();

            unsafe {
                for _ in 0..max_frames_in_flight {
                    frame_fences.push(
                        render_context.get_device().borrow().get().create_fence(
                            &fence_create,
                            None)
                            .expect("Failed to create Frame fence")
                    );

                    render_semaphores.push(
                        render_context.get_device().borrow().get().create_semaphore(
                            &semaphore_create, None)
                            .expect("Failed to create Render semaphore")
                    );
                }
            }
        }

        let immediate_command_buffer = render_context.get_immediate_command_buffer();

        let imgui_renderer = {
            let font_texture = {
                let fonts = imgui.fonts();
                fonts.build_rgba32_texture()
            };

            ImguiRender::new(
                render_context.get_device().clone(),
                &render_context,
                &immediate_command_buffer,
                font_texture)
        };

        let examples: Vec<Box<dyn Example>> = vec![
            Box::new(UboExample::new(render_context.get_device().clone())),
            Box::new(ModelExample::new(render_context.get_device().clone(), &render_context, &immediate_command_buffer))
        ];

        let mut frames: Vec<Option<Box<Frame>>> = Vec::new();
        frames.resize_with(max_frames_in_flight as usize, Default::default);

        WindowedVulkanApp {
            window,
            platform,
            examples: Examples::new(examples),
            imgui,
            frame_graph,
            imgui_renderer,
            render_semaphores,
            frames,
            frame_fences,
            frame_index: 0,
            render_context,
            tracy
        }
    }

    pub fn shutdown(&mut self) {
        println!("Shutting down");
        unsafe {
            let device = self.render_context.get_device();
            device.borrow().get()
                .device_wait_idle()
                .expect("Failed to wait for GPU to be idle");

            for semaphore in &self.render_semaphores {
                device.borrow().get().destroy_semaphore(*semaphore, None);
            }

            for fence in &self.frame_fences {
                device.borrow().get().destroy_fence(*fence, None);
            }

        }
    }

    #[tracing::instrument]
    pub fn draw_frame(&mut self) {
        // wait for fence if necessary (can we avoid this using just semaphores?)
        let frame_fence = self.frame_fences[self.frame_index as usize];
        let wait_fences = [frame_fence];
        log::trace!(target: "frame", "Waiting for frame: {}", self.frame_index);
        unsafe {
            let _span = tracy_client::span!("Wait on Frame fence");
            self.render_context.get_device().borrow().get()
                .wait_for_fences(
                    // std::slice::from_ref(&wait_fence),
                    &wait_fences,
                    true,
                    u64::MAX)
                .expect("Failed to wait for Frame Fence");
        }
        log::trace!(target: "frame", "Wait complete; cleaning up frame.");
        // clean up the completed frame
        self.frames[self.frame_index as usize] = None;

        self.render_context.start_frame(self.frame_index);

        // get swapchain image for this frame
        let VulkanFrameObjects {
            graphics_command_buffer: command_buffer,
            swapchain_image,
            swapchain_semaphore,
            descriptor_pool,
            frame_index: render_ctx_frame_index,
            ..
        } = self.render_context.get_next_frame_objects();

        let next_image = match &swapchain_image {
            Some(next_image) => {
                next_image.image.as_ref().unwrap().clone()
            }
            None => {
                panic!("No swapchain exists")
            }
        };

        // begin commandbuffer
        unsafe {
            let _span = tracy_client::span!("Begin commandbuffer");
            self.render_context.get_device().borrow().get().reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::empty())
                .expect("Failed to reset command buffer");
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE)
                .build();
            self.render_context.get_device().borrow().get().begin_command_buffer(command_buffer, &begin_info)
                .expect("Failed to begin recording command buffer");
        }

        // update imgui UI
        let ui = self.imgui.new_frame();
        {
            let _span = tracy_client::span!("Build UI");
            if let Some(main_menu) = ui.begin_main_menu_bar() {
                if let Some(file_menu) = ui.begin_menu("File") {}
                if let Some(examples_menu) = ui.begin_menu("Examples") {
                    for (i, example) in &mut self.examples.examples.iter().enumerate() {
                        if ui.menu_item(example.get_name()) {
                            self.examples.active_example_index = Some(i);
                        }
                    }
                }
            }
        }

        // prepare framegraph
        log::trace!(target: "frame", "Creating new frame: {}", self.frame_index);
        self.frames[self.frame_index as usize] = Some(self.frame_graph.start(self.render_context.get_device(), descriptor_pool));
        let current_frame = self.frames[self.frame_index as usize].as_mut().unwrap();

        {
            let _span = tracy_client::span!("Build Framegraph");
            {
                let present_node = PresentPassNode::builder("present".to_string())
                    .swapchain_image(next_image.clone())
                    .build()
                    .expect("Failed to create Present Node");

                current_frame.start(PassType::Present(present_node));
            }

            {
                let clear_node = clear::clear(next_image.clone(), vk::ImageAspectFlags::COLOR);
                current_frame.add_node(clear_node);
            }

            {
                let rt_ref = AttachmentReference::new(
                    next_image.clone(),
                    vk::SampleCountFlags::TYPE_1);

                if let Some(index) = self.examples.active_example_index {
                    if let Some(active_example) = self.examples.examples.get(index) {
                        let nodes = active_example.execute(
                            self.render_context.get_device(),
                            ui,
                            rt_ref.clone());
                        for node in nodes {
                            current_frame.add_node(node);
                        }
                    }
                }

                let imgui_draw_data = self.imgui.render();

                let imgui_nodes = self.imgui_renderer.generate_passes(
                    imgui_draw_data,
                    rt_ref.clone(),
                    self.render_context.get_device());

                for imgui_node in imgui_nodes {
                    current_frame.add_node(imgui_node);
                }
            }
        }

        self.frame_graph.end(
            current_frame,
            &mut self.render_context,
            &command_buffer);

        // end command buffer
        // TODO: support multiple command buffers
        unsafe {
            self.render_context.get_device().borrow().get().end_command_buffer(command_buffer)
                .expect("Failed to finish recording command buffer");
        }

        // queue submit
        {
            unsafe {
                let fences_to_reset = [frame_fence];
                self.render_context.get_device().borrow().get()
                    // .reset_fences(std::slice::from_ref(&frame_fence))
                    .reset_fences(&fences_to_reset)
                    .expect("Failed to reset Frame Fence");
            }

            self.render_context.submit_graphics(
                &[command_buffer],
                frame_fence,
                &[swapchain_semaphore],
                &[self.render_semaphores[self.frame_index as usize]]);
        }

        // prepare present
        // flip
        {
            let _span = tracy_client::span!("Present");

            let swapchain_status = self.render_context.flip(
                &[self.render_semaphores[self.frame_index as usize]]);

            self.render_context.end_frame();


            if swapchain_status == SwapchainStatus::Suboptimal {
                self.render_context.recreate_swapchain(&self.window);
            }
        }
        self.tracy.frame_mark();

        let max_frames_in_flight = {
            match self.render_context.get_swapchain() {
                Some(swapchain) => {
                    swapchain.get_images().len() as u32
                }
                None => {
                    MAX_FRAMES_IN_FLIGHT
                }
            }
        };
        self.frame_index = (self.frame_index + 1) % max_frames_in_flight;

    }
}

#[tracing::instrument]
fn run(mut app: WindowedVulkanApp, event_loop: EventLoop<()>) -> Result<(), EventLoopError> {
    let mut last_frame = Instant::now();

    // &self.event_loop.run(move |event, _, control_flow| {
    event_loop.run(move |event, event_loop| {
        let _span = tracy_client::span!("Event Loop");
        match event {
            Event::NewEvents(_) => {
                let now = Instant::now();
                app.imgui.io_mut().update_delta_time(now - last_frame);
                last_frame = now;
            },
            Event::AboutToWait => {
                app.platform.prepare_frame(app.imgui.io_mut(), &app.window)
                    .expect("Failed to prepare frame");
                app.window.request_redraw();
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                event_loop.exit();
            },
            Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                app.draw_frame();
            },
            Event::LoopExiting => {
                app.shutdown();
            },
            event => {
                app.platform.handle_event(app.imgui.io_mut(), &app.window, &event);
            }
        }
    })
}

fn main() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(tracing_tracy::TracyLayer::default())
    ).expect("setup tracy layer");

    // create app
    let event_loop: EventLoop<()> = EventLoop::new().expect("Couldn't create EventLoop");
    let app = WindowedVulkanApp::new(&event_loop, "Examples", 1200, 800);
    run(app, event_loop);

}