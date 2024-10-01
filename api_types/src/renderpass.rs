use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::interface::DeviceInterface;

#[derive(Clone)]
pub struct DeviceRenderpass<'a> {
    pub renderpass: vk::RenderPass,
    pub device: &'a DeviceInterface
}

impl Drop for DeviceRenderpass {
    fn drop(&mut self) {
        unsafe {
            *self.device.destroy_render_pass(self.renderpass, None);
        }
    }
}

impl DeviceRenderpass {
    pub fn new(
        renderpass: vk::RenderPass,
        device: &DeviceInterface) -> Self {

        DeviceRenderpass {
            renderpass,
            device
        }
    }
}
