use ash::vk;
use crate::resource::vulkan_resource_manager::ResourceHandle;

pub struct AttachmentReference {
    pub handle: ResourceHandle,
    pub samples: vk::SampleCountFlags,
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub last_usage: vk::ImageLayout
}