use crate::resource::resource_manager::{ResourceHandle, ResolvedResource};

pub trait ResourceManager {

    fn resolve_resource(&mut self, handle: &ResourceHandle) -> ResolvedResource;

}