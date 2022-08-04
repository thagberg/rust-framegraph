use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResource};

pub trait ResourceManager {

    fn resolve_resource(&mut self, handle: &ResourceHandle) -> ResolvedResource;

}