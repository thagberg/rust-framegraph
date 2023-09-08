use std::ffi::CString;
use std::time::Instant;
use ash::vk;

use winit;
use winit::window::{Window, WindowBuilder};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use imgui;
use context::render_context::RenderContext;
use context::vulkan_render_context::{VulkanFrameObjects, VulkanRenderContext};
use framegraph::frame::Frame;
use framegraph::frame_graph::FrameGraph;
use framegraph::pipeline::VulkanPipelineManager;
use framegraph::renderpass_manager::VulkanRenderpassManager;
use framegraph::vulkan_frame_graph::VulkanFrameGraph;
use passes::imgui_draw::ImguiRender;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct WindowedVulkanApp {
    window: Window,
    platform: WinitPlatform,
    imgui: imgui::Context,

    render_context: VulkanRenderContext,
    frame_graph: VulkanFrameGraph,

    imgui_renderer: ImguiRender,

    frames: [Option<Box<Frame>>; MAX_FRAMES_IN_FLIGHT]
}

impl WindowedVulkanApp {
    pub fn new(event_loop: &EventLoop<()>, title: &str, width: u32, height: u32) -> WindowedVulkanApp {
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .build(event_loop)
            .expect("Failed to create window");

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        let render_context = {
            let c_title = CString::new(title).unwrap();
            let application_info = vk::ApplicationInfo::builder()
                .application_name(&c_title);

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

        WindowedVulkanApp {
            window,
            platform,
            imgui,
            render_context,
            frame_graph,
            imgui_renderer,
            frames: Default::default(),
        }
    }

    pub fn draw_frame(&mut self) {
        // wait for fence if necessary (can we avoid this using just semaphores?)

        // get swapchain image for this frame
        let VulkanFrameObjects {
            graphics_command_buffer: command_buffer,
            swapchain_image: swapchain_image,
            frame_index: frame_index
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
        let imgui_draw_data = {
            let ui = self.imgui.new_frame();
            ui.text("Testing UI");

            self.imgui.render()
        };

        // prepare framegraph
        self.frames[frame_index as usize] = Some(self.frame_graph.start());
        let current_frame = self.frames[frame_index as usize].as_mut().unwrap();

        {
            let imgui_nodes = self.imgui_renderer.generate_passes(
                imgui_draw_data,
                swapchain_image.unwrap(),
                self.render_context.get_device());

            for (i, imgui_node) in imgui_nodes.into_iter().enumerate() {
                if i == 0 {
                    current_frame.start(imgui_node);
                } else {
                    current_frame.add_node(imgui_node);
                }
            }
        }
        self.frame_graph.end(
            current_frame,
            &mut self.render_context,
            &command_buffer);

        // queue submit

        // prepare present

        // queue present (wait on semaphores)
    }

    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<u32, &'static str>{
        let mut last_frame = Instant::now();

        // &self.event_loop.run(move |event, _, control_flow| {
        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::NewEvents(_) => {
                    let now = Instant::now();
                    self.imgui.io_mut().update_delta_time(now - last_frame);
                    last_frame = now;
                },
                Event::MainEventsCleared => {
                    self.platform.prepare_frame(self.imgui.io_mut(), &self.window)
                        .expect("Failed to prepare frame");
                    self.window.request_redraw();
                },
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    *control_flow = ControlFlow::Exit;
                },
                Event::RedrawRequested(_) => {
                    self.draw_frame();
                },
                event => {
                    self.platform.handle_event(self.imgui.io_mut(), &self.window, &event);
                }
            }
        });

        Ok(0)
    }
}

fn main() {
    // create app
    let event_loop: EventLoop<()> = EventLoop::new();
    let app = WindowedVulkanApp::new(&event_loop, "Examples", 1200, 800);
    let exit = app.run(event_loop);

}