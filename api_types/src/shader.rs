use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::DeviceWrapper;

#[derive(Clone)]
pub struct DeviceShader {
    pub shader_module: vk::ShaderModule,
    pub device: Arc<Mutex<DeviceWrapper>>
}

impl Drop for DeviceShader {
    fn drop(&mut self) {
        unsafe {
            self.device.lock()
                .expect("Failed to obtain device lock")
                .get().destroy_shader_module(self.shader_module, None)
        }
    }
}

impl DeviceShader {
    pub fn new(shader_module: vk::ShaderModule, device: Arc<Mutex<DeviceWrapper>>) -> Self {
        DeviceShader {
            shader_module,
            device
        }
    }
}
