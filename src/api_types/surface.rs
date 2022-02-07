use ash::vk;

pub struct SurfaceWrapper {
    surface: vk::SurfaceKHR,
    surface_loader: ash::extensions::khr::Surface
}

impl SurfaceWrapper {
    pub fn new(
        surface: vk::SurfaceKHR,
        surface_loader: ash::extensions::khr::Surface
    ) -> SurfaceWrapper {
        SurfaceWrapper {
            surface,
            surface_loader
        }
    }

    pub fn get_surface(&self) -> &vk::SurfaceKHR {
        &self.surface
    }

    pub fn get_loader(&self) -> &ash::extensions::khr::Surface {
        &self.surface_loader
    }
}

impl Drop for SurfaceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}
