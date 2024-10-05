use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::interface::DeviceInterface;

#[derive(Clone)]
pub struct DeviceRenderpass<'a> {
    pub renderpass: vk::RenderPass,
    pub device: &'a DeviceInterface
}

impl Drop for DeviceRenderpass<'_> {
    fn drop(&mut self) {
        unsafe {
            self.device.get().destroy_render_pass(self.renderpass, None);
        }
    }
}

impl<'a> DeviceRenderpass<'a> {
    pub fn new(
        renderpass: vk::RenderPass,
        device: &'a DeviceInterface) -> Self {

        DeviceRenderpass {
            renderpass,
            device
        }
    }
}
