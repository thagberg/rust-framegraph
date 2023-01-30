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

pub(crate) struct ResolvedResource {
    handle: ResourceHandle,
    resource: ResourceType
}

struct ResolvedResourceInternal {
    resource: ResourceType,
    allocation: Allocation
}

pub type ResolvedResourceMap = HashMap<ResourceHandle, ResolvedResource>;
type ResolvedResourceInternalMap = HashMap<ResourceHandle, ResolvedResourceInternal>;

pub struct VulkanResourceManager {
    next_handle: RefCell<u32>,
    allocator: Allocator,
    resource_map: RefCell<ResolvedResourceInternalMap>,
    /// Registered resources are those created / managed elsewhere but for which we still want
    /// a resource handle and to be resolveable (such as swapchain images)
    registered_resource_map: ResolvedResourceMap,
    device: Rc<DeviceWrapper>
}

fn create_image(
    allocator: &mut Allocator,
    device: &DeviceWrapper,
    create_info: &ImageCreateInfo) -> ResolvedResourceInternal
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

    ResolvedResourceInternal {
        resource: ResourceType::Image(image),
        allocation: image_alloc
    }
}

fn create_buffer(
    allocator: &mut Allocator,
    device: &DeviceWrapper,
    create_info: &BufferCreateInfo) -> ResolvedResourceInternal {

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

    ResolvedResourceInternal {
        resource: ResourceType::Buffer(buffer),
        allocation: Default::default()
    }
}

impl ResourceManager for VulkanResourceManager {
    fn resolve_resource(
        &self,
        handle: &ResourceHandle) -> ResolvedResource
    {
        let resolved = {
            // first check the resource_map, if not found there look in the registered_resource_map
            // most resources will not be registered resources, so it should be checked last
            let found = self.resource_map.borrow().get(handle);
            if None = found {
                self.registered_resource_map.get(handle)
                    .expect("Attempted to resolve a resource which doesn't exist")
            } else {
                found.unwrap()
            }
        };

        ResolvedResource {
            handle: *handle,
            resource: *resolved.resource
        }
    }

    fn reset(&mut self, device: &DeviceWrapper) {
        for (handle, resolved) in self.resource_map.borrow_mut().drain() {
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
            resource_map: RefCell::new(HashMap::new()),
            registered_resource_map: HashMap::new(),
            device
        }
    }

    pub fn reserve_handle(&self) -> ResourceHandle {
        let handle = self.next_handle.borrow_mut();
        handle.replace(*handle+1)
    }

    pub fn create_buffer(
        &mut self,
        create_info: BufferCreateInfo
    ) -> ResourceHandle {
        let handle = self.next_handle.borrow_mut();
        let ret_handle = handle.replace(*handle+1);

        let resolved_buffer = create_buffer(&mut self.allocator, &self.device, &create_info);
        self.resource_map.insert(ret_handle, resolved_buffer);

        ret_handle
    }

    pub fn update_buffer<F>(
        &mut self,
        buffer: &ResourceHandle,
        mut fill_callback: F)
        where F: FnMut(*mut c_void)
    {
        let resolved_resource = self.resource_map.get(buffer)
            .expect("Failed to resolve buffer for update");
        if let ResourceType::Buffer(buffer) = &resolved_resource.resource {
            let alloc = &resolved_resource.allocation;
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
        } else {
            panic!("Attempting to update a non-buffer resource as a buffer");
        }
    }

    pub fn register_image(
        &mut self,
        image: &ImageWrapper,
        name: &str
    ) -> ResourceHandle
    {
        let handle = self.next_handle.borrow_mut();
        let ret_handle = handle.replace(*handle+1);

        self.registered_resource_map.insert(
            ret_handle,
            ResolvedResource {
                handle: ret_handle,
                resource: ResourceType::Image(image.clone())
            });

        self.device.set_image_name(image, name);

        ret_handle
    }


}