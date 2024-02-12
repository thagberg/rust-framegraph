use std::os::raw::c_void;
use ash::extensions::khr::Win32Surface;
use ash::vk;

use crate::api_types::device::PhysicalDeviceWrapper;

pub struct SurfaceWrapper {
    surface: vk::SurfaceKHR,
    surface_loader: ash::extensions::khr::Surface
}

pub struct SurfaceCapabilities {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>
}

impl SurfaceCapabilities {
    pub fn new(
        capabilities: vk::SurfaceCapabilitiesKHR,
        formats: Vec<vk::SurfaceFormatKHR>,
        present_modes: Vec<vk::PresentModeKHR>
    ) -> SurfaceCapabilities {
        SurfaceCapabilities {
            capabilities,
            formats,
            present_modes
        }
    }
}

impl SurfaceWrapper {
    pub fn new(
        entry: &ash::Entry,
        instance: &ash::Instance,
        window: &winit::window::Window
    ) -> SurfaceWrapper {
        let surface = unsafe {
            ash_window::create_surface(entry, instance, &window, None)
                .expect("Failed to create window surface")
        };
        let surface_loader = ash::extensions::khr::Surface::new(entry, instance);

        SurfaceWrapper {
            surface,
            surface_loader
        }
    }

    pub fn get_surface(&self) -> vk::SurfaceKHR {
        self.surface
    }

    pub fn get_loader(&self) -> &ash::extensions::khr::Surface {
        &self.surface_loader
    }

    pub fn get_surface_capabilities(&self,
        physical_device: &PhysicalDeviceWrapper,
        surface: &SurfaceWrapper
    ) -> SurfaceCapabilities {
        unsafe {
            let capabilities = surface.get_loader()
                .get_physical_device_surface_capabilities(
                    physical_device.get(),
                    surface.get_surface())
                .expect("Failed to query device for surface capabilities.");

            let formats = surface.get_loader()
                .get_physical_device_surface_formats(
                    physical_device.get(),
                    surface.get_surface())
                .expect("Failed to query for surface formats.");

            let present_modes = surface.get_loader()
                .get_physical_device_surface_present_modes(
                    physical_device.get(),
                    surface.get_surface() )
                .expect("Failed to query surface for present modes.");

            SurfaceCapabilities::new(capabilities, formats, present_modes)
        }
    }
}

impl Drop for SurfaceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}
