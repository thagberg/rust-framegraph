use ash::vk;
use crate::resource::vulkan_resource_manager::ResourceHandle;

pub struct AttachmentReference {
    pub handle: ResourceHandle,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlags,
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub layout: vk::ImageLayout
}

impl AttachmentReference {
    pub fn new(
        handle: ResourceHandle,
        format: vk::Format,
        samples: vk::SampleCountFlags,
        load_op: vk::AttachmentLoadOp,
        store_op: vk::AttachmentStoreOp) -> AttachmentReference {

        AttachmentReference {
            handle,
            format,
            samples,
            load_op,
            store_op,
            layout: vk::ImageLayout::UNDEFINED
        }
    }
}