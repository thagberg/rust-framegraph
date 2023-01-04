use ash::vk;
use crate::resource::vulkan_resource_manager::{ResolvedResource, ResourceHandle, ResourceType};

#[derive(Clone)]
pub struct ImageBindingInfo {
    pub sampler: vk::Sampler,
    pub last_usage: vk::ImageLayout
}

#[derive(Clone)]
pub struct BufferBindingInfo {
    pub offset: vk::DeviceSize,
    pub range: vk::DeviceSize
}

#[derive(Clone)]
pub enum BindingType {
    Buffer(BufferBindingInfo),
    Image(ImageBindingInfo)
}

#[derive(Clone)]
pub struct BindingInfo {
    pub binding_type: BindingType,
    pub set: u64,
    pub slot: u32
}

#[derive(Clone)]
pub struct ResourceBinding {
    pub handle: ResourceHandle,
    pub binding_info: BindingInfo
}

pub struct ResolvedResourceBinding {
    pub resolved_resource: ResolvedResource
}
