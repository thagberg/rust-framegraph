use std::collections::HashMap;
use core::ffi::c_void;
use ash::vk;
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;

extern crate context;
use context::api_types::device::{PhysicalDeviceWrapper, DeviceWrapper};
use context::api_types::image::ImageWrapper;

use crate::resource::resource_manager::{ResourceManager};

#[derive(Clone, Copy, Hash, std::cmp::Eq)]
pub enum ResourceHandle {
    Transient(u32),
    Persistent(u32)
}

impl PartialEq for ResourceHandle {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ResourceHandle::Transient(x), ResourceHandle::Transient(y)) => x == y,
            (ResourceHandle::Persistent(x), ResourceHandle::Persistent(y)) => x == y,
            _ => false
        }
    }
}

pub enum ResourceCreateInfo {
    Buffer(vk::BufferCreateInfo),
    Image(vk::ImageCreateInfo)
}

// #[derive(Clone, Copy)]
#[derive(Clone)]
pub enum ResourceType {
    Buffer(vk::Buffer),
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

// #[derive(Clone, Copy)]
#[derive(Clone)]
pub struct ResolvedResource {
    pub handle: ResourceHandle,
    pub resource: ResourceType
}

pub struct ResolvedBuffer {
    buffer: vk::Buffer,
    allocation: Allocation
}

pub struct ResolvedImage {
    image: ImageWrapper,
    allocation: Allocation
}

pub type ResolvedResourceMap = HashMap<ResourceHandle, ResolvedResource>;

pub struct VulkanResourceManager<'a> {
    next_handle: u32,
    allocator: Allocator,
    transient_resource_map: HashMap<ResourceHandle, TransientResource>,
    resolved_resource_map: ResolvedResourceMap,
    persistent_resource_map: HashMap<ResourceHandle, PersistentResource>,
    device: &'a DeviceWrapper
}

impl ResolvedBuffer {
    pub fn get(&self) -> vk::Buffer { self.buffer }
    pub fn get_allocation(&self) -> &Allocation { &self.allocation }
}

impl ResourceManager for VulkanResourceManager<'_> {
    fn resolve_resource(
        &mut self,
        handle: &ResourceHandle) -> ResolvedResource
    {
        match handle {
            ResourceHandle::Transient(_) => {
                let resolved = self.resolved_resource_map.get(handle);
                match resolved {
                    Some(found) => { found.clone() },
                    None => {
                        let transient = self.transient_resource_map.get(handle)
                            .expect("No transient resource found");
                        let resolved_resource: Option<ResolvedResource>;
                        match transient.create_info {
                            ResourceCreateInfo::Buffer(buffer_create) => {
                                let resolved_buffer = self.create_resolved_buffer(&buffer_create);
                                resolved_resource = Some(ResolvedResource {
                                    handle: handle.clone(),
                                    resource: ResourceType::Buffer(resolved_buffer.buffer)
                                });
                            },
                            ResourceCreateInfo::Image(image_create) => {
                                let resolved_image = self.create_resolved_image(&image_create);
                                resolved_resource = Some(ResolvedResource {
                                    handle: handle.clone(),
                                    resource: ResourceType::Image(resolved_image.image)
                                });
                            }
                        }

                        match resolved_resource {
                            Some(rr) => {
                                self.resolved_resource_map.insert(handle.clone(), rr.clone());
                                rr
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
                        Some(&found.create_info)
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
}

impl<'a> VulkanResourceManager<'a> {
    pub fn new(
        instance: &ash::Instance,
        device: &'a DeviceWrapper,
        physical_device: &PhysicalDeviceWrapper
    ) -> VulkanResourceManager<'a> {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.get().clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false // TODO: what is this
        }).expect("Failed to create GPU memory allocator");

        VulkanResourceManager {
            next_handle: 0,
            allocator,
            transient_resource_map: HashMap::new(),
            resolved_resource_map: HashMap::new(),
            persistent_resource_map: HashMap::new(),
            device
        }
    }


    pub fn create_buffer_transient(
        &mut self,
        create_info: vk::BufferCreateInfo
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
        create_info: vk::ImageCreateInfo
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

    fn create_resolved_image(
        &mut self,
        create_info: &vk::ImageCreateInfo
    ) -> ResolvedImage {
        self.create_image(create_info)
    }

    fn create_resolved_buffer(
        &mut self,
        create_info: &vk::BufferCreateInfo
    ) -> ResolvedBuffer {
       self.create_uniform_buffer(create_info)
    }

    pub fn create_buffer_persistent(
        &mut self,
        create_info: &vk::BufferCreateInfo
    ) -> ResourceHandle {
        let ret_handle = ResourceHandle::Persistent(self.next_handle);
        self.next_handle += 1;

        let resolved_buffer = self.create_uniform_buffer(create_info);

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

    fn create_image(
        &mut self,
        create_info: &vk::ImageCreateInfo
    ) -> ResolvedImage
    {
        let mut image_alloc: Allocation = Default::default();
        let image = self.device.create_image(
            create_info,
            &mut |memory_requirements: vk::MemoryRequirements| -> (vk::DeviceMemory, vk::DeviceSize) {
                unsafe {
                    image_alloc = self.allocator.allocate(&AllocationCreateDesc {
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

    pub fn register_image(
        &mut self,
        image: &ImageWrapper
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

        ret_handle
    }

    fn create_uniform_buffer(
        &mut self,
        create_info: &vk::BufferCreateInfo
    ) -> ResolvedBuffer {
        let buffer = unsafe {
            self.device.get().create_buffer(create_info, None)
                .expect("Failed to create uniform buffer")
        };
        let requirements = unsafe {
            self.device.get().get_buffer_memory_requirements(buffer)
        };

        let buffer_alloc = self.allocator.allocate(&AllocationCreateDesc {
            name: "Uniform Buffer Allocation",
            requirements: requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true
        }).expect("Failed to allocate memory for uniform buffer");

        unsafe {
            self.device.get().bind_buffer_memory(
                buffer,
                buffer_alloc.memory(),
                buffer_alloc.offset())
                .expect("Failed to bind uniform buffer to memory")
        };

        ResolvedBuffer {
            buffer,
            allocation: buffer_alloc
        }
    }

}