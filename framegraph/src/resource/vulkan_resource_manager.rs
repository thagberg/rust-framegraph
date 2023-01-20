use std::collections::HashMap;
use core::ffi::c_void;
use std::cell::RefCell;
use std::rc::Rc;
use ash::{Device, vk};
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;

extern crate context;
use context::api_types::device::{PhysicalDeviceWrapper, DeviceWrapper};
use context::api_types::image::{ImageCreateInfo, ImageWrapper};
use context::api_types::buffer::{BufferCreateInfo, BufferWrapper};

use crate::resource::resource_manager::{ResourceManager};

pub type ResourceHandle = u32;

    Image {
        handle: ResourceHandle
    }
}

pub enum ResourceCreateInfo {
    Buffer(BufferCreateInfo),
    Image(ImageCreateInfo)
}

// #[derive(Clone, Copy)]
#[derive(Clone)]
pub enum ResourceType {
    Buffer(BufferWrapper),
    Image(ImageWrapper)
}

pub struct TransientResource {
    handle: ResourceHandle,
    create_info: ResourceCreateInfo
}

pub struct PersistentResource {
    pub handle: ResourceHandle,
    pub resource: ResourceType,
    pub allocation: Allocation
}

pub struct ResolvedResourceInternal {
    resource: ResolvedResource,
    allocation: Allocation
}

#[derive(Clone)]
pub struct ResolvedResource {
    pub handle: ResourceHandle,
    pub resource: ResourceType
}

pub struct ResolvedBuffer {
    buffer: BufferWrapper,
    allocation: Allocation
}

pub struct ResolvedImage {
    image: ImageWrapper,
    allocation: Allocation
}

pub type ResolvedResourceMap = HashMap<ResourceHandle, ResolvedResource>;
type ResolvedResourceInternalMap = HashMap<ResourceHandle, ResolvedResourceInternal>;

pub struct VulkanResourceManager {
    next_handle: RefCell<u32>,
    allocator: Allocator,
    transient_resource_map: HashMap<ResourceHandle, TransientResource>,
    resolved_resource_map: ResolvedResourceInternalMap,
    persistent_resource_map: HashMap<ResourceHandle, PersistentResource>,
    device: Rc<DeviceWrapper>
}

fn create_resolved_image(
    allocator: &mut Allocator,
    device: &DeviceWrapper,
    create_info: &ImageCreateInfo) -> ResolvedImage {
    create_image(allocator, device, create_info)
}

fn create_image(
    allocator: &mut Allocator,
    device: &DeviceWrapper,
    create_info: &ImageCreateInfo) -> ResolvedImage
{
    let mut image_alloc: Allocation = Default::default();
    let image = device.create_image(
        create_info,
        &mut |memory_requirements: vk::MemoryRequirements| -> (vk::DeviceMemory, vk::DeviceSize) {
            unsafe {
                image_alloc = allocator.allocate(&AllocationCreateDesc {
                    name: "Image allocation",
                    requirements: memory_requirements,
                    location: MemoryLocation::GpuOnly, // TODO: Parameterized eventually?
                    linear: true // TODO: I think this is required for render targets?
                }).expect("Failed to allocate memory for image");
                (image_alloc.memory(), image_alloc.offset())
            }
        });

    ResolvedImage {
        image,
        allocation: image_alloc
    }
}

fn create_resolved_buffer(
    allocator: &mut Allocator,
    device: &DeviceWrapper,
    create_info: &BufferCreateInfo) -> ResolvedBuffer {
    create_uniform_buffer(allocator, device, create_info)
}

fn create_uniform_buffer(
    allocator: &mut Allocator,
    device: &DeviceWrapper,
    create_info: &BufferCreateInfo) -> ResolvedBuffer {

    let mut buffer_alloc: Allocation = Default::default();
    let buffer = device.create_buffer(
        create_info,
        &mut |memory_requirements: vk::MemoryRequirements| -> (vk::DeviceMemory, vk::DeviceSize) {
            unsafe {
                buffer_alloc = allocator.allocate(&AllocationCreateDesc {
                    name: "Uniform Buffer Allocation", // TODO: use the create_info name here?
                    requirements: memory_requirements,
                    location: MemoryLocation::CpuToGpu, // TODO: should definitely parameterize this
                    linear: true
                }).expect("Failed to allocate memory for buffer");
                (buffer_alloc.memory(), buffer_alloc.offset())
            }
        }
    );

    ResolvedBuffer {
        buffer,
        allocation: buffer_alloc
    }
}

impl ResourceManager for VulkanResourceManager {
    fn resolve_resource(
        &mut self,
        handle: &ResourceHandle) -> ResolvedResource
    {
        match handle {
            ResourceHandle::Transient(_) => {
                let resolved = self.resolved_resource_map.get(handle);
                match resolved {
                    Some(found) => {
                        found.resource.clone()
                    },
                    None => {
                        let transient = self.transient_resource_map.get(handle)
                            .expect("No transient resource found");

                        let resolved_resource: Option<ResolvedResourceInternal>;
                        match &transient.create_info {
                            ResourceCreateInfo::Buffer(buffer_create) => {
                                let resolved_buffer = create_resolved_buffer(&mut self.allocator, &self.device, buffer_create);
                                resolved_resource = Some(ResolvedResourceInternal {
                                    resource: ResolvedResource {
                                        handle: handle.clone(),
                                        resource: ResourceType::Buffer(resolved_buffer.buffer)
                                    },
                                    allocation: resolved_buffer.allocation
                                });
                            },
                            ResourceCreateInfo::Image(image_create) => {
                                let resolved_image = create_resolved_image(&mut self.allocator, &self.device, image_create);
                                resolved_resource = Some(ResolvedResourceInternal {
                                    resource: ResolvedResource {
                                        handle: handle.clone(),
                                        resource: ResourceType::Image(resolved_image.image)
                                    },
                                    allocation: resolved_image.allocation
                                });
                            }
                        }

                        match resolved_resource {
                            Some(rr) => {
                                let return_resource = rr.resource.clone();
                                self.resolved_resource_map.insert(handle.clone(), rr);
                                return_resource
                            },
                            None => {
                                panic!("Failed to find or create resource during resolution");
                            }
                        }
                        // resolved_resource
                        //     .expect("Failed to create transient resource")
                    }
                }
                // let resolved = self.transient_resource_map.get(handle)
                //     .expect("Transient resource was not added");
            },
            ResourceHandle::Persistent(_) => {
                let resolved = self.persistent_resource_map.get(handle)
                    .expect("Need to handle not found resources");
                ResolvedResource {
                    handle: handle.clone(),
                    resource: resolved.resource.clone()
                }
            }
        }
    }

    fn get_resource_description(&self, handle: &ResourceHandle) -> Option<&ResourceCreateInfo> {
        match handle {
            ResourceHandle::Transient(t) => {
                let resource = self.transient_resource_map.get(handle);
                match resource {
                    Some(found) => {
                        return Some(&found.create_info);
                    },
                    _ => {
                        panic!("Trying to get description of non-existant resource");
                    }
                }
            },
            ResourceHandle::Persistent(p) => {
                panic!("get_resource_description not implemented for persistent resources");
            }
        }

        None
    }

    fn flush(&mut self, device: &DeviceWrapper) {
        for (handle, resolved) in self.resolved_resource_map.drain() {
            match &resolved.resource.resource {
                ResourceType::Image(resolved_image) => {
                    unsafe {
                        device.get().destroy_image_view(resolved_image.view, None);
                        device.get().destroy_image(resolved_image.image, None);
                        self.allocator.free(resolved.allocation)
                            .expect("Failed to free image allocation");
                    }
                },
                ResourceType::Buffer(resolved_buffer) => {
                    unsafe {
                        device.get().destroy_buffer(resolved_buffer.buffer, None);
                        self.allocator.free(resolved.allocation)
                            .expect("Failed to free buffer allocation");
                    }
                }
            }
        }

        self.transient_resource_map.clear();
    }
}

impl VulkanResourceManager {
    pub fn new(
        instance: &ash::Instance,
        device: Rc<DeviceWrapper>,
        physical_device: &PhysicalDeviceWrapper
    ) -> VulkanResourceManager {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.get().clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false // TODO: what is this
        }).expect("Failed to create GPU memory allocator");

        VulkanResourceManager {
            next_handle: RefCell::new(0u32),
            allocator,
            transient_resource_map: HashMap::new(),
            resolved_resource_map: HashMap::new(),
            persistent_resource_map: HashMap::new(),
            device
        }
    }

    pub fn reserve_handle(&self) -> ResourceHandle {
        let handle = self.next_handle.borrow_mut();
        handle.replace(*handle+1)
    }

    pub fn create_buffer_transient(
        &mut self,
        create_info: BufferCreateInfo
    ) -> ResourceHandle {
        let ret_handle = ResourceHandle::Transient(self.next_handle);
        self.next_handle += 1;

        self.transient_resource_map.insert(ret_handle, TransientResource {
            handle: ret_handle,
            create_info: ResourceCreateInfo::Buffer(create_info)
        });

        ret_handle
    }

    pub fn create_image_transient(
        &mut self,
        create_info: ImageCreateInfo
    ) -> ResourceHandle
    {
        let ret_handle = ResourceHandle::Transient(self.next_handle);
        self.next_handle += 1;

        self.transient_resource_map.insert(ret_handle, TransientResource {
            handle: ret_handle,
            create_info: ResourceCreateInfo::Image(create_info)
        });

        ret_handle
    }


    pub fn create_buffer_persistent(
        &mut self,
        create_info: BufferCreateInfo
    ) -> ResourceHandle {
        let ret_handle = ResourceHandle::Persistent(self.next_handle);
        self.next_handle += 1;

        let resolved_buffer = create_uniform_buffer(&mut self.allocator, &self.device, &create_info);

        self.persistent_resource_map.insert(ret_handle, PersistentResource {
            handle: ret_handle,
            resource: ResourceType::Buffer(resolved_buffer.buffer),
            allocation: resolved_buffer.allocation
        });

        ret_handle
    }

    pub fn update_buffer<F>(
        &mut self,
        buffer: &ResourceHandle,
        mut fill_callback: F)
        where F: FnMut(*mut c_void)
    {
        match buffer {
            ResourceHandle::Transient(_) => panic!("Can't update transient buffer"),
            ResourceHandle::Persistent(handle) => {
                let find = self.persistent_resource_map.get(buffer);
                match find {
                    None => panic!("Buffer doesn't exist"),
                    Some(resolved_buffer) => {
                        let alloc = &resolved_buffer.allocation;
                        if alloc.mapped_ptr().is_some() {
                            fill_callback(alloc.mapped_ptr().unwrap().as_ptr());
                        } else {
                            unsafe {
                                let mapped_memory = self.device.get().map_memory(
                                    alloc.memory(),
                                    alloc.offset(),
                                    alloc.size(),
                                    vk::MemoryMapFlags::empty() )
                                    .expect("Failed to map buffer");
                                fill_callback(mapped_memory);
                                self.device.get().unmap_memory(alloc.memory());
                            }
                        }
                    }
                }
            }
        }
    }


    pub fn register_image(
        &mut self,
        image: &ImageWrapper,
        name: &str
    ) -> ResourceHandle
    {
        let ret_handle = ResourceHandle::Persistent(self.next_handle);
        self.next_handle += 1;

        self.persistent_resource_map.insert(ret_handle, PersistentResource {
            handle: ret_handle,
            resource: ResourceType::Image(image.clone()),
            // allocation: resolved_buffer.allocation
            allocation: Allocation::default() // TODO: this is really dumb and bad
        });

        self.device.set_image_name(image, name);

        ret_handle
    }


}