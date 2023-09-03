use std::time::Instant;

use winit;
use winit::window::{Window, WindowBuilder};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use imgui;

struct WindowedVulkanApp {
    window: Window,
    platform: WinitPlatform,
    imgui: imgui::Context
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

        let entry = ash::Entry::linked();

        WindowedVulkanApp {
            window,
            platform,
            imgui
        }
    }

    pub fn draw_frame(&self) {

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