use ash::vk;
use crate::device::interface::DeviceInterface;

#[derive(Clone)]
pub struct DeviceShader {
    pub shader_module: vk::ShaderModule,
    pub device: DeviceInterface
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
        device: DeviceInterface) -> Self {
        DeviceShader {
            shader_module,
            device
        }
    }
}
