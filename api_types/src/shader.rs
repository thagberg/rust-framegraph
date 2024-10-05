use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::interface::DeviceInterface;

#[derive(Clone)]
pub struct DeviceShader<'a> {
    pub shader_module: vk::ShaderModule,
    pub device: &'a DeviceInterface
}

impl Drop for DeviceShader<'_> {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_shader_module(self.shader_module, None)
        }
    }
}

impl<'a> DeviceShader<'a> {
    pub fn new(
        shader_module: vk::ShaderModule,
        device: &'a DeviceInterface) -> Self {
        DeviceShader {
            shader_module,
            device
        }
    }
}
