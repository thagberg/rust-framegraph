use ash::vk;
use crate::resource::vulkan_resource_manager::ResourceHandle;

pub struct ImageBarrier {
    pub handle: ResourceHandle,
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub source_access: vk::AccessFlags,
    pub dest_access: vk::AccessFlags,
    pub old_layout: vk::ImageLayout,
    pub new_layout: vk::ImageLayout
}

pub struct BufferBarrier {
    pub handle: ResourceHandle,
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub source_access: vk::AccessFlags,
    pub dest_access: vk::AccessFlags,
    pub size: usize,
    pub offset: usize
}
