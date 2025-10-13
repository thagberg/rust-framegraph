use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::interface::DeviceInterface;

pub struct DeviceFramebuffer {
    framebuffer: vk::Framebuffer,
    device: DeviceInterface
}

impl Drop for DeviceFramebuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.get().destroy_framebuffer(self.framebuffer, None);
        }
    }
}

impl DeviceFramebuffer {
    pub fn new(framebuffer: vk::Framebuffer, device: DeviceInterface) -> Self {
        DeviceFramebuffer {
            framebuffer: framebuffer,
            device: device
        }
    }

    pub fn get_framebuffer(&self) -> vk::Framebuffer { self.framebuffer }
}
