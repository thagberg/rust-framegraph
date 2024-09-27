use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::DeviceResource;

#[derive(Clone)]
pub struct ImageBindingInfo {
    pub layout: vk::ImageLayout
}

impl Debug for ImageBindingInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageBindingInfo")
            .finish()
    }
}

#[derive(Clone)]
pub struct BufferBindingInfo {
    pub offset: vk::DeviceSize,
    pub range: vk::DeviceSize
}
impl Debug for BufferBindingInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferBindingInfo")
            .field("range", &self.range)
            .field("offset", &self.offset)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub enum BindingType {
    Buffer(BufferBindingInfo),
    Image(ImageBindingInfo)
}

#[derive(Clone)]
pub struct BindingInfo {
    pub binding_type: BindingType,
    pub set: u64,
    pub slot: u32,
    pub stage: vk::PipelineStageFlags,
    pub access: vk::AccessFlags
}

impl Debug for BindingInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindingInfo")
            .field("binding type", &self.binding_type)
            .field("set", &self.set)
            .field("slot", &self.slot)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct ResourceBinding {
    pub resource: Arc<Mutex<DeviceResource>>,
    pub binding_info: BindingInfo
}
