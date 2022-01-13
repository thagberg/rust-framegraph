use winit::event::{Event, VirtualKeyCode, KeyboardInput, WindowEvent, ElementState};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::Window;

use ash::vk;
use ash::{Device, Instance};
use ash::extensions::{
    ext::DebugUtils,
    khr::{Surface, Swapchain},
};

use std::ffi::{CStr, CString};
use std::ptr;

const WINDOW_TITLE: &'static str = "Vulkan Framegraph";
const WINDOW_HEIGHT: u32 = 1000;
const WINDOW_WIDTH: u32 = 1400;

struct WindowedApp {
    // instance: ash::Instance
    instance: Instance
}

impl WindowedApp {
    pub fn new(event_loop: &EventLoop<()>, window: &Window) -> WindowedApp {
        let entry = ash::Entry::linked();
        // let instance = WindowedApp::create_instance(&entry, &None);
        let instance = WindowedApp::create_instance(&entry, window);

        WindowedApp {
            instance
        }
    }

    fn init_window(event_loop: &EventLoop<()>) -> winit::window::Window {
        winit::window::WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .build(event_loop)
            .expect("Failed to initialize window")
    }

    fn select_device(instance: &Instance) -> vk::PhysicalDevice {
        unsafe {
            let physical_devices = instance
                .enumerate_physical_devices()
                .expect("Cannot find physical devices");

            let (p_device, queue_family_index) = physical_devices
                .iter()
                .map(|p_device| {

                })
                .flatten()
                .next()
                .expect("Couldn't find suitable physical device");
        }
    }

    fn create_instance(entry: &ash::Entry, window: &Window) -> Instance{
        let engine_name = CString::new("Framegraph").unwrap();
        let app_name = CString::new(WINDOW_TITLE).unwrap();
        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: std::ptr::null(),
            p_application_name: app_name.as_ptr(),
            application_version: vk::make_api_version(0, 0, 1, 0),
            p_engine_name: engine_name.as_ptr(),
            engine_version: vk::make_api_version(0, 0, 1, 0),
            api_version: vk::make_api_version(0, 1, 2, 198)
        };

        let validation_layer = CString::new("VK_LAYER_KHRONOS_validation").unwrap();
        let layer_names = [validation_layer.as_ptr()];

        let mut extension_names_raw = ash_window::enumerate_required_extensions(&window).unwrap()
            .iter()
            .map(|extension| extension.as_ptr())
            .collect::<Vec<_>>();
        extension_names_raw.push(DebugUtils::name().as_ptr());

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_layer_names(&layer_names)
            .enabled_extension_names(&extension_names_raw);

        let mut instance;
        unsafe {
            instance = entry
                .create_instance(&create_info, None)
                .expect("Failed to create Vulkan Instance");
        }

        instance
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
                                }
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
    //WindowedApp::main_loop(event_loop, window);
    let app = WindowedApp::new(&event_loop, &window);
}
