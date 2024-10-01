use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::DeviceWrapper;

#[derive(Clone)]
pub struct DeviceRenderpass {
    pub renderpass: vk::RenderPass,
    pub device: Arc<Mutex<DeviceWrapper>>
}

impl Drop for DeviceRenderpass {
    fn drop(&mut self) {
        unsafe {
            self.device.lock()
                .expect("Failed to obtain device lock.")
                .get().destroy_render_pass(self.renderpass, None);
        }
    }
}

impl DeviceRenderpass {
    pub fn new(
        renderpass: vk::RenderPass,
        device: Arc<Mutex<DeviceWrapper>>) -> Self {

        DeviceRenderpass {
            renderpass,
            device
        }
    }
}
