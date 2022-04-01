use std::collections::HashMap;
use core::ffi::c_void;
use ash::vk;
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;
use crate::api_types::device::PhysicalDeviceWrapper;
use crate::DeviceWrapper;

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

#[derive(Clone, Copy)]
pub enum ResourceType {
    Buffer(vk::Buffer),
    Image(vk::Image)
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

#[derive(Clone, Copy)]
pub struct ResolvedResource {
    pub handle: ResourceHandle,
    pub resource: ResourceType
}

pub struct ResolvedBuffer {
    buffer: vk::Buffer,
    allocation: Allocation
}

pub struct ResourceManager {
    next_handle: u32,
    allocator: Allocator,
    transient_resource_map: HashMap<ResourceHandle, TransientResource>,
    persistent_resource_map: HashMap<ResourceHandle, PersistentResource>
}

impl ResolvedBuffer {
    pub fn get(&self) -> vk::Buffer { self.buffer }
    pub fn get_allocation(&self) -> &Allocation { &self.allocation }
}

impl ResourceManager {
    pub fn new(
        instance: &ash::Instance,
        device: &DeviceWrapper,
        physical_device: &PhysicalDeviceWrapper
    ) -> ResourceManager {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.get().clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false // TODO: what is this
        }).expect("Failed to create GPU memory allocator");

        ResourceManager {
            next_handle: 0,
            allocator,
            transient_resource_map: HashMap::new(),
            persistent_resource_map: HashMap::new()
        }
    }

    pub fn resolve_resource(&self, handle: &ResourceHandle) -> ResolvedResource {
        match handle {
            ResourceHandle::Transient(_) => {
                panic!("Need to implement this for transient resources");
            },
            ResourceHandle::Persistent(_) => {
                let resolved = self.persistent_resource_map.get(handle)
                    .expect("Need to handle not found resources");
                ResolvedResource {
                    handle: handle.clone(),
                    resource: resolved.resource
                }
            }
        }
    }

    pub fn create_buffer_transient(
        &mut self,
        create_info: &vk::BufferCreateInfo
    ) -> ResourceHandle {
        let ret_handle = ResourceHandle::Transient(self.next_handle);
        self.next_handle += 1;

        self.transient_resource_map.insert(ret_handle, TransientResource {
            handle: ret_handle,
            create_info: ResourceCreateInfo::Buffer(create_info.clone())
        });

        ret_handle
    }

    pub fn create_buffer_persistent(
        &mut self,
        device: &DeviceWrapper,
        create_info: &vk::BufferCreateInfo
    ) -> ResourceHandle {
        let ret_handle = ResourceHandle::Persistent(self.next_handle);
        self.next_handle += 1;

        let resolved_buffer = self.create_uniform_buffer(device, create_info);

        self.persistent_resource_map.insert(ret_handle, PersistentResource {
            handle: ret_handle,
            resource: ResourceType::Buffer(resolved_buffer.buffer),
            allocation: resolved_buffer.allocation
        });

        ret_handle
    }

    pub fn update_buffer<F>(
        &mut self,
        device: &DeviceWrapper,
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
                                let mapped_memory = device.get().map_memory(
                                    alloc.memory(),
                                    alloc.offset(),
                                    alloc.size(),
                                    vk::MemoryMapFlags::empty() )
                                    .expect("Failed to map buffer");
                                fill_callback(mapped_memory);
                                device.get().unmap_memory(alloc.memory());
                            }
                        }
                    }
                }
            }
        }
    }

    fn create_uniform_buffer(
        &mut self,
        device: &DeviceWrapper,
        create_info: &vk::BufferCreateInfo
    ) -> ResolvedBuffer {
        // let create_info = vk::BufferCreateInfo {
        //     s_type: vk::StructureType::BUFFER_CREATE_INFO,
        //     size,
        //     usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
        //     sharing_mode: vk::SharingMode::EXCLUSIVE,
        //     ..Default::default()
        // };

        let buffer = unsafe {
            device.get().create_buffer(create_info, None)
                .expect("Failed to create uniform buffer")
        };
        let requirements = unsafe {
            device.get().get_buffer_memory_requirements(buffer)
        };

        let buffer_alloc = self.allocator.allocate(&AllocationCreateDesc {
            name: "Uniform Buffer Allocation",
            requirements: requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true
        }).expect("Failed to allocate memory for uniform buffer");

        unsafe {
            device.get().bind_buffer_memory(
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