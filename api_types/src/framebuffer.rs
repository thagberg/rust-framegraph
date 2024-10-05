use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::interface::DeviceInterface;

pub struct DeviceFramebuffer<'a> {
    framebuffer: vk::Framebuffer,
    device: &'a DeviceInterface
}

impl Drop for DeviceFramebuffer<'_> {
    fn drop(&mut self) {
        unsafe {
            self.device.get().destroy_framebuffer(self.framebuffer, None);
        }
    }
}

impl<'a> DeviceFramebuffer<'a> {
    pub fn new(framebuffer: vk::Framebuffer, device: &'a DeviceInterface) -> Self {
        DeviceFramebuffer {
            framebuffer: framebuffer,
            device: device
        }
    }

    pub fn get_framebuffer(&self) -> vk::Framebuffer { self.framebuffer }
}
