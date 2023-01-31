extern crate context;
use context::api_types::device::DeviceWrapper;
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResource, ResourceCreateInfo};


pub trait ResourceManager {

    fn resolve_resource(&self, handle: &ResourceHandle) -> ResolvedResource;

    fn reset(&mut self, device: &DeviceWrapper);
}