use crate::resource::vulkan_resource_manager::{ResourceHandle, ResourceType};

#[derive(Clone)]
pub struct ResourceBinding {
    pub handle: ResourceHandle,
    pub resource_type: ResourceType,
    pub binding: u32
}

