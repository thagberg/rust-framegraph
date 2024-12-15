use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::resource::DeviceResource;

pub struct ImageBarrier<'a> {
    pub resource: Arc<Mutex<DeviceResource<'a>>>,
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub source_access: vk::AccessFlags,
    pub dest_access: vk::AccessFlags,
    pub old_layout: vk::ImageLayout,
    pub new_layout: vk::ImageLayout
}

pub struct BufferBarrier<'a> {
    pub resource: Arc<Mutex<DeviceResource<'a>>>,
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub source_access: vk::AccessFlags,
    pub dest_access: vk::AccessFlags,
    pub size: usize,
    pub offset: usize
}
