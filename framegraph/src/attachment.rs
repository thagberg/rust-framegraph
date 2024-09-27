use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::{DeviceResource, ResourceType};

#[derive(Clone)]
pub struct AttachmentReference {
    pub resource_image: Arc<Mutex<DeviceResource>>,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlags,
    pub layout: vk::ImageLayout
}

impl AttachmentReference {
    pub fn new(
        resource_image: Arc<Mutex<DeviceResource>>,
        samples: vk::SampleCountFlags) -> AttachmentReference {

        assert!(resource_image.borrow().resource_type.is_some(), "AttachmentResource: resource_image must be valid DeviceResource");
        let resource_ref = resource_image.borrow();
        let resolved_resource = resource_ref.resource_type.as_ref().expect("AttachmentResource: resource_image must be valid resource");
        if let ResourceType::Buffer(_) = &resolved_resource {
            assert!(false, "AttachmentResource: resource_image must be an Image type");
        }

        AttachmentReference {
            resource_image: resource_image.clone(),
            format: resource_image.borrow().get_image().format,
            samples,
            layout: vk::ImageLayout::UNDEFINED
        }
    }
}