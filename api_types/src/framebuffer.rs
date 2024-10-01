use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::DeviceWrapper;

pub struct DeviceFramebuffer {
    framebuffer: vk::Framebuffer,
    device: Arc<Mutex<DeviceWrapper>>
}

impl Drop for DeviceFramebuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.lock()
                .expect("Failed to obtain device lock.")
                .get().destroy_framebuffer(
                self.framebuffer, None);
        }
    }
}

impl DeviceFramebuffer {
    pub fn new(framebuffer: vk::Framebuffer, device: Arc<Mutex<DeviceWrapper>>) -> Self {
        DeviceFramebuffer {
            framebuffer: framebuffer,
            device: device
        }
    }

    pub fn get_framebuffer(&self) -> vk::Framebuffer { self.framebuffer }
}
