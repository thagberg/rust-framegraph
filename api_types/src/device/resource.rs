use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use gpu_allocator::vulkan::{Allocation, Allocator};
use crate::buffer::BufferWrapper;
use crate::device::allocator::ResourceAllocator;
use crate::device::interface::DeviceInterface;
use crate::image::ImageWrapper;

#[derive(Clone)]
pub enum ResourceType<'m> {
    Buffer(BufferWrapper<'m>),
    Image(ImageWrapper)
}

pub struct DeviceResource<'a, 'm> where 'a : 'm{
    pub allocation: Option<Allocation>,
    pub resource_type: Option<ResourceType<'m>>,

    handle: u64,
    device: &'a DeviceInterface,
    allocator: Option<Arc<Mutex<ResourceAllocator>>>
}

impl Debug for DeviceResource<'_, '_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceResource")
            .field("handle", &self.handle)
            .finish()
    }
}

impl Drop for DeviceResource<'_, '_> {
    fn drop(&mut self) {
        if let Some(resource_type) = &mut self.resource_type {
            match resource_type {
                ResourceType::Buffer(buffer) => {
                    log::trace!(target: "resource", "Destroying buffer: {}", self.handle);
                    self.device.destroy_buffer(buffer);
                },
                ResourceType::Image(image) => {
                    log::trace!(target: "resource", "Destroying image: {}", self.handle);
                    self.device.destroy_image(image);
                }
            }
        }
        if let Some(alloc) = &mut self.allocation {
            let moved = std::mem::replace(alloc, Allocation::default());
            if let Some(allocator) = &self.allocator {
                let mut allocator_ref = allocator.lock().unwrap();
                allocator_ref.free_allocation(moved);
            }
        }
    }
}

impl PartialEq<Self> for DeviceResource<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl Eq for DeviceResource<'_, '_> {}

impl<'a, 'm> DeviceResource<'a, 'm> {

    pub(crate) fn new(
        allocation: Option<Allocation>,
        resource_type: Option<ResourceType<'m>>,
        handle: u64,
        device: &'a DeviceInterface,
        allocator: Option<Arc<Mutex<ResourceAllocator>>>
    ) -> Self {
        DeviceResource {
            allocation,
            resource_type,
            handle,
            device,
            allocator
        }
    }

    pub fn get_image(&self) -> &ImageWrapper {
        match &self.resource_type {
            Some(resolved_resource) => {
                match &resolved_resource {
                    ResourceType::Image(image) => {
                        image
                    },
                    _ => {
                        panic!("Non-image resource type")
                    }
                }
            },
            None => {
                panic!("Unresolved resource")
            }
        }
    }

    pub fn get_image_mut(&mut self) -> &mut ImageWrapper {
        match self.resource_type.as_mut() {
            Some(resolved_resource) => {
                match resolved_resource {
                    ResourceType::Image(image) => {
                        image
                    },
                    _ => {
                        panic!("Non-image resource type")
                    }
                }
            },
            None => {
                panic!("Unresolved resource")
            }
        }
    }

    pub fn get_buffer(&self) -> &BufferWrapper {
        match &self.resource_type {
            Some(resolved_resource) => {
                match &resolved_resource {
                    ResourceType::Buffer(buffer) => {
                        buffer
                    },
                    _ => {
                        panic!("Non-buffer resource type")
                    }
                }
            },
            None => {
                panic!("Unresolved resource")
            }
        }
    }

    pub fn get_handle(&self) -> u64 {
        self.handle
    }
}

