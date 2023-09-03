use winit;
use winit::window::{Window, WindowBuilder};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use imgui;

struct WindowedVulkanApp {
    window: Window,
    platform: WinitPlatform,
    event_loop: EventLoop<()>,
    imgui: imgui::Context
}

impl WindowedVulkanApp {
    pub fn new(title: &str, width: u32, height: u32) -> WindowedVulkanApp {
        let event_loop: EventLoop<()> = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .build(&event_loop)
            .expect("Failed to create window");

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        WindowedVulkanApp {
            window,
            platform,
            event_loop,
            imgui
        }
    }

    pub fn run(mut self) -> Result<u32, &'static str>{
        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    *control_flow = ControlFlow::Exit;
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
    let mut app = WindowedVulkanApp::new("Examples", 1200, 800);
    let exit = app.run();

}