use winit::event::{Event, VirtualKeyCode, KeyboardInput, WindowEvent, ElementState};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::Window;

use ash::vk;

use std::ffi::{CStr, CString};
use std::ptr;

const WINDOW_TITLE: &'static str = "Vulkan Framegraph";
const WINDOW_HEIGHT: u32 = 1000;
const WINDOW_WIDTH: u32 = 1400;

struct WindowedApp {
    // instance: ash::Instance
}

impl WindowedApp {
    pub fn new() -> WindowedApp {
        let entry = ash::Entry::linked();
        // let instance = WindowedApp::create_instance(&entry, &None);
        WindowedApp::create_instance(&entry, &None);

        WindowedApp {
            // instance
        }
    }

    fn init_window(event_loop: &EventLoop<()>) -> winit::window::Window {
        winit::window::WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .build(event_loop)
            .expect("Failed to initialize window")
    }

    // fn create_instance(entry: &ash::Entry, window: &Option<Window>) -> ash::Instance {
    fn create_instance(entry: &ash::Entry, window: &Option<Window>) {
        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: std::ptr::null(),
            p_application_name: CString::new(WINDOW_TITLE).unwrap().as_ptr(),
            application_version: vk::make_api_version(0, 0, 1, 0),
            p_engine_name: CString::new("Framegraph").unwrap().as_ptr(),
            engine_version: vk::make_api_version(0, 0, 1, 0),
            api_version: vk::make_api_version(0, 1, 2, 198)
        };

        let layer_names = [CString::new("VK_LAYER_KHRONOS_validation").unwrap().as_ptr()];

        //let extension_names = [ash_window::enumerate_required_extensions()]
        // let extension_names = match window {
        //     Some(window) => [ash_window::enumerate_required_extensions(window).unwrap()],
        //     None => []
        // };
        let mut extension_names: Vec<&CStr>;
        match window {
            Some(window) => extension_names = ash_window::enumerate_required_extensions(window).unwrap(),
            None => {},
        }

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_layer_names(&layer_names)
            .enabled_extension_names(&extension_names[..]);

    }

    fn main_loop(event_loop: EventLoop<()>) {
        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit
                        },
                        WindowEvent::KeyboardInput { input, .. } => {
                            match input {
                                KeyboardInput { virtual_keycode, state, .. } => {
                                    match (virtual_keycode, state) {
                                        (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                                            dbg!();
                                            *control_flow = ControlFlow::Exit
                                        },
                                        _ => {},
                                    }
                                },
                                _ => {},
                            }
                        }
                        _ => {},
                    }
                },
                _ => {},
            }
        })
    }
}

fn main() {
    println!("Hello, world!");
    let event_loop = EventLoop::new();
    let window = WindowedApp::init_window(&event_loop);
    WindowedApp::main_loop(event_loop);
}
