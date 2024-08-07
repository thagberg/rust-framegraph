use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use api_types::device::DeviceResource;

pub struct ImageBarrier {
    pub resource: Rc<RefCell<DeviceResource>>,
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub source_access: vk::AccessFlags,
    pub dest_access: vk::AccessFlags,
    pub old_layout: vk::ImageLayout,
    pub new_layout: vk::ImageLayout
}

pub struct BufferBarrier {
    pub resource: Rc<RefCell<DeviceResource>>,
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub source_access: vk::AccessFlags,
    pub dest_access: vk::AccessFlags,
    pub size: usize,
    pub offset: usize
}
