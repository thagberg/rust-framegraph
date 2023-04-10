use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::{DeviceResource, ResourceType};
use crate::resource::vulkan_resource_manager::{ResolvedResource, ResourceHandle};

#[derive(Clone)]
pub struct ImageBindingInfo {
    pub sampler: vk::Sampler,
    pub layout: vk::ImageLayout
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
    pub slot: u32,
    pub stage: vk::PipelineStageFlags,
    pub access: vk::AccessFlags
}

#[derive(Clone)]
pub struct ResourceBinding {
    pub resource: Rc<RefCell<DeviceResource>>,
    pub binding_info: BindingInfo
}

pub struct ResolvedResourceBinding {
    pub resolved_resource: ResolvedResource
}
