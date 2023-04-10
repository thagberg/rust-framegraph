use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::{DeviceResource, ResourceType};
use crate::resource::vulkan_resource_manager::ResourceHandle;

pub struct AttachmentReference {
    pub resource_image: Rc<RefCell<DeviceResource>>,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlags,
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub layout: vk::ImageLayout
}

impl AttachmentReference {
    pub fn new(
        resource_image: Rc<RefCell<DeviceResource>>,
        format: vk::Format,
        samples: vk::SampleCountFlags,
        load_op: vk::AttachmentLoadOp,
        store_op: vk::AttachmentStoreOp) -> AttachmentReference {

        assert!(resource_image.borrow().resource_type.is_some(), "AttachmentResource: resource_image must be valid DeviceResource");
        let resource_ref = resource_image.borrow();
        let resolved_resource = resource_ref.resource_type.as_ref().expect(("AttachmentResource: resource_image must be valid resource"));
        if let ResourceType::Buffer(buffer) = &resolved_resource {
            assert!(false, "AttachmentResource: resource_image must be an Image type");
        }

        AttachmentReference {
            resource_image: resource_image.clone(),
            format,
            samples,
            load_op,
            store_op,
            layout: vk::ImageLayout::UNDEFINED
        }
    }
}