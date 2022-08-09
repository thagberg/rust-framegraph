use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResource, ResourceCreateInfo};

pub trait ResourceManager {

    fn resolve_resource(&mut self, handle: &ResourceHandle) -> ResolvedResource;

    fn get_resource_description(&self, handle: &ResourceHandle) -> Option<&ResourceCreateInfo>;
}