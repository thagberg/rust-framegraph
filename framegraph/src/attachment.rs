use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::resource::{DeviceResource, ResourceType};

#[derive(Clone)]
pub struct AttachmentReference<'a> {
    pub resource_image: Arc<Mutex<DeviceResource<'a>>>,
    pub format: vk::Format,
    pub samples: vk::SampleCountFlags,
    pub layout: vk::ImageLayout
}

impl<'a> AttachmentReference<'a> {
    pub fn new(
        resource_image: Arc<Mutex<DeviceResource<'a>>>,
        samples: vk::SampleCountFlags) -> AttachmentReference<'a> {

        assert!(resource_image.lock().unwrap().resource_type.is_some(), "AttachmentResource: resource_image must be valid DeviceResource");
        let resource_ref = resource_image.lock().unwrap();
        let format = resource_ref.get_image().format;
        let resolved_resource = resource_ref.resource_type.as_ref().expect("AttachmentResource: resource_image must be valid resource");
        if let ResourceType::Buffer(_) = &resolved_resource {
            assert!(false, "AttachmentResource: resource_image must be an Image type");
        }

        AttachmentReference {
            resource_image: resource_image.clone(),
            format,
            samples,
            layout: vk::ImageLayout::UNDEFINED
        }
    }
}