use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use gpu_allocator::vulkan::Allocation;
use crate::buffer::BufferWrapper;
use crate::device::DeviceWrapper;
use crate::image::ImageWrapper;

#[derive(Clone)]
pub enum ResourceType {
    Buffer(BufferWrapper),
    Image(ImageWrapper)
}

pub struct DeviceResource {
    pub allocation: Option<Allocation>,
    pub resource_type: Option<ResourceType>,

    handle: u64,
    device: Arc<Mutex<DeviceWrapper>>
}

impl Debug for DeviceResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceResource")
            .field("handle", &self.handle)
            .finish()
    }
}

impl Drop for DeviceResource {
    fn drop(&mut self) {
        let mut device = self.device.lock()
            .expect("Failed to lock device when dropping resource");
        if let Some(resource_type) = &mut self.resource_type {
            match resource_type {
                ResourceType::Buffer(buffer) => {
                    log::trace!(target: "resource", "Destroying buffer: {}", self.handle);
                    device.destroy_buffer(buffer);
                },
                ResourceType::Image(image) => {
                    log::trace!(target: "resource", "Destroying image: {}", self.handle);
                    device.destroy_image(image);
                }
            }
        }
        if let Some(alloc) = &mut self.allocation {
            let moved = std::mem::replace(alloc, Allocation::default());
            device.free_allocation(moved);
        }
    }
}

impl PartialEq<Self> for DeviceResource {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl Eq for DeviceResource {}

impl DeviceResource {
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

