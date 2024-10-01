use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::DeviceWrapper;
use crate::device::interface::DeviceInterface;

#[derive(Clone)]
pub struct DeviceShader<'a> {
    pub shader_module: vk::ShaderModule,
    pub device: &'a DeviceInterface
}

impl Drop for DeviceShader {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_shader_module(self.shader_module, None)
        }
    }
}

impl DeviceShader {
    pub fn new(
        shader_module: vk::ShaderModule,
        device: &DeviceInterface) -> Self {
        DeviceShader {
            shader_module,
            device
        }
    }
}
