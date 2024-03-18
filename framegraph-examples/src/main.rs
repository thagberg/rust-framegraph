mod ubo_example;
mod example;
mod model_example;

extern crate alloc;
extern crate nalgebra_glm as glm;

//use alloc::ffi::CString;
use std::ffi::CString;
use std::time::Instant;
use ash::vk;

use winit;
use winit::window::{Window, WindowBuilder};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use imgui;
use imgui::BackendFlags;
use winit::error::EventLoopError;
use context::api_types::swapchain::SwapchainWrapper;
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

    render_context: VulkanRenderContext
}

impl WindowedVulkanApp {
    pub fn new(event_loop: &EventLoop<()>, title: &str, width: u32, height: u32) -> WindowedVulkanApp {
        let window = WindowBuilder::new()
            .with_title(title)
            // .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window");

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        imgui.io_mut().backend_flags |= BackendFlags::RENDERER_HAS_VTX_OFFSET;

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        let render_context = {
            let c_title = CString::new(title).unwrap();
            let application_info = vk::ApplicationInfo::builder()
                .application_name(&c_title)
                .api_version(vk::API_VERSION_1_2);

            VulkanRenderContext::new(
                &application_info,
                true,
                Some(&window))
        };

        let frame_graph = VulkanFrameGraph::new(
            VulkanRenderpassManager::new(),
            VulkanPipelineManager::new());

        let imgui_renderer = {
            let font_texture = {
                let fonts = imgui.fonts();
                fonts.build_rgba32_texture()
            };

            ImguiRender::new(
                render_context.get_device().clone(),
                &render_context,
                font_texture)
        };

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

        let examples: Vec<Box<dyn Example>> = vec![
            Box::new(UboExample::new(render_context.get_device().clone())),
            Box::new(ModelExample::new(render_context.get_device().clone()))
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

    pub fn draw_frame(&mut self) {
        println!("New frame");
        // wait for fence if necessary (can we avoid this using just semaphores?)
        let wait_fence = self.frame_fences[self.frame_index as usize];
        unsafe {
            self.render_context.get_device().borrow().get()
                .wait_for_fences(
                    std::slice::from_ref(&wait_fence),
                    true,
                    u64::MAX)
                .expect("Failed to wait for Frame Fence");

            // self.render_context.get_device().borrow().get()
            //     .device_wait_idle()
            //     .expect("Failed to idle");
        }
        // clean up the completed frame
        self.frames[self.frame_index as usize] = None;

        // get swapchain image for this frame
        let VulkanFrameObjects {
            graphics_command_buffer: command_buffer,
            swapchain_image,
            swapchain_semaphore,
            descriptor_pool,
            frame_index: swapchain_index,
        } = self.render_context.get_next_frame_objects();

        // begin commandbuffer
        unsafe {
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
        let mut opened = true;
        if let Some(main_menu) = ui.begin_main_menu_bar() {
            if let Some(file_menu) = ui.begin_menu("File") {

            }
            if let Some(examples_menu) = ui.begin_menu("Examples") {
                for (i, example) in &mut self.examples.examples.iter().enumerate() {
                    if ui.menu_item(example.get_name()) {
                        self.examples.active_example_index = Some(i);
                    }
                }
            }
        }

        // prepare framegraph
        self.frames[self.frame_index as usize] = Some(self.frame_graph.start(self.render_context.get_device(), descriptor_pool));
        let current_frame = self.frames[self.frame_index as usize].as_mut().unwrap();

        if let Some(swapchain_image) = swapchain_image.clone() {
            let present_node = PresentPassNode::builder("present".to_string())
                .swapchain_image(swapchain_image)
                .build()
                .expect("Failed to create Present Node");

            current_frame.start(PassType::Present(present_node));
        }

        {
            let clear_node = clear::clear_color(swapchain_image.as_ref().unwrap().clone());
            current_frame.add_node(clear_node);
        }


        {
            let image = swapchain_image.unwrap();
            let rt_ref = AttachmentReference::new(
                image.clone(),
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
                self.render_context.get_device().borrow().get()
                    .reset_fences(std::slice::from_ref(&wait_fence))
                    .expect("Failed to reset Frame Fence");
            }

            self.render_context.submit_graphics(
                &[command_buffer],
                wait_fence,
                &[swapchain_semaphore],
                &[self.render_semaphores[self.frame_index as usize]]);
        }

        // prepare present
        // flip
        {
            self.render_context.flip(
                &[self.render_semaphores[self.frame_index as usize]],
                swapchain_index);

            let swapchain = self.render_context.get_swapchain().as_ref().unwrap().get();

            // let present = vk::PresentInfoKHR::builder()
            //     .wait_semaphores()
        }

        // queue present (wait on semaphores)

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

    // pub fn run(mut self, event_loop: EventLoop<()>) -> Result<u32, &'static str>{
    //     let mut last_frame = Instant::now();
    //
    //     // &self.event_loop.run(move |event, _, control_flow| {
    //     event_loop.run(move |event, _, control_flow| {
    //         match event {
    //             Event::NewEvents(_) => {
    //                 let now = Instant::now();
    //                 self.imgui.io_mut().update_delta_time(now - last_frame);
    //                 last_frame = now;
    //             },
    //             Event::MainEventsCleared => {
    //                 self.platform.prepare_frame(self.imgui.io_mut(), &self.window)
    //                     .expect("Failed to prepare frame");
    //                 self.window.request_redraw();
    //             },
    //             Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
    //                 *control_flow = ControlFlow::Exit;
    //             },
    //             Event::RedrawRequested(_) => {
    //                 self.draw_frame();
    //             },
    //             Event::LoopDestroyed => {
    //                 self.shutdown();
    //             },
    //             event => {
    //                 self.platform.handle_event(self.imgui.io_mut(), &self.window, &event);
    //             }
    //         }
    //     });
    //
    //     Ok(0)
    // }
}

fn run(mut app: WindowedVulkanApp, event_loop: EventLoop<()>) -> Result<(), EventLoopError> {
    let mut last_frame = Instant::now();

    // &self.event_loop.run(move |event, _, control_flow| {
    event_loop.run(move |event, event_loop| {
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
    // create app
    let event_loop: EventLoop<()> = EventLoop::new().expect("Couldn't create EventLoop");
    let app = WindowedVulkanApp::new(&event_loop, "Examples", 1200, 800);
    // let exit = app.run(event_loop);
    run(app, event_loop);

}