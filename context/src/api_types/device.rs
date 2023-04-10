use std::cell::RefCell;
use std::ffi::{CStr, CString};
use core::ffi::c_void;
use std::rc::Rc;
use ash::vk;
use ash::extensions::ext::DebugUtils;
use ash::vk::{DebugUtilsObjectNameInfoEXT, Handle};
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;
use crate::api_types;

use crate::api_types::image::{ImageWrapper, ImageCreateInfo};
use crate::api_types::buffer::{BufferWrapper, BufferCreateInfo};

#[derive(Copy, Clone)]
pub struct QueueFamilies {
    pub graphics: Option<u32>,
    pub compute: Option<u32>,
    pub present: Option<u32>
}

impl QueueFamilies {
    pub fn is_complete(&self) -> bool {
        self.graphics.is_some() && self.compute.is_some() && self.present.is_some()
    }
}

#[derive(Copy, Clone)]
pub struct PhysicalDeviceWrapper {
    physical_device: vk::PhysicalDevice,
}

impl PhysicalDeviceWrapper {
    pub fn new(physical_device: vk::PhysicalDevice) -> PhysicalDeviceWrapper {
        PhysicalDeviceWrapper {
            physical_device
        }
    }

    pub fn get(&self) -> vk::PhysicalDevice { self.physical_device }
}

pub struct DeviceWrapper {
    device: ash::Device,
    debug_utils: DebugUtils,
    queue_family_indices: QueueFamilies,
    allocator: Allocator,
    handle_generator: u64
}

#[derive(Clone)]
pub enum ResourceType {
    Buffer(BufferWrapper),
    Image(ImageWrapper)
}

pub struct DeviceResource {
    pub allocation: Option<Allocation>,
    pub resource_type: Option<ResourceType>,

    handle: u64,
    device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DeviceResource {
    fn drop(&mut self) {
        let allocation = {
        };
        if let Some(resource_type) = &mut self.resource_type {
            match resource_type {
                ResourceType::Buffer(buffer) => {
                    self.device.borrow_mut().destroy_buffer(buffer);
                },
                ResourceType::Image(image) => {
                    self.device.borrow_mut().destroy_image(image);
                }
            }
        }
        if let Some(alloc) = &mut self.allocation {
            let moved = std::mem::replace(alloc, Allocation::default());
            self.device.borrow_mut().free_allocation(moved);
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
    pub fn get_handle(&self) -> u64 {
        self.handle
    }
}

impl Drop for DeviceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.allocator.report_memory_leaks(log::Level::Warn);
            self.device.destroy_device(None);
        }
    }
}

impl DeviceWrapper {
    pub fn new(
        device: ash::Device,
        instance: &ash::Instance,
        physical_device: &PhysicalDeviceWrapper,
        debug_utils: DebugUtils,
        queue_family_indices: QueueFamilies) -> DeviceWrapper {

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false // https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPhysicalDeviceBufferDeviceAddressFeaturesEXT.html
        }).expect("Failed to create GPU memory allocator");

        DeviceWrapper {
            device,
            debug_utils,
            queue_family_indices,
            allocator,
            handle_generator: 0
        }
    }
    pub fn get(&self) -> &ash::Device {
        &self.device
    }
    pub fn get_queue_family_indices(&self) -> &QueueFamilies { &self.queue_family_indices }

    pub fn free_allocation(&mut self, allocation: Allocation) {
        unsafe {
            self.allocator.free(allocation)
                .expect("Failed to free Device allocation");
        }
    }

    pub fn destroy_buffer(&mut self, buffer: &BufferWrapper) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
        }
    }

    pub fn destroy_image(&mut self, image: &ImageWrapper) {
        unsafe {
            self.device.destroy_image_view(image.view, None);
            self.device.destroy_image(image.image, None);
        }
    }

    pub fn create_image_view(
        &self,
        image: vk::Image,
        format: vk::Format,
        image_view_flags: vk::ImageViewCreateFlags,
        aspect_flags: vk::ImageAspectFlags,
        mip_levels: u32) -> vk::ImageView
    {
        let create_info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: image_view_flags,
            view_type: vk::ImageViewType::TYPE_2D,
            format,
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY
            },
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: aspect_flags,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1
            },
            image: image
        };

        unsafe {
            self.device.create_image_view(&create_info, None)
                .expect("Failed to create image view.")
        }
    }

    fn set_debug_name(&self, object_type: vk::ObjectType, handle: u64, name: &str)
    {
        let c_name = CString::new(name)
            .expect("Failed to create C-name for debug object");
        let debug_info = DebugUtilsObjectNameInfoEXT::builder()
            .object_type(object_type)
            .object_handle(handle)
            .object_name(&c_name)
            .build();
        unsafe {
            self.debug_utils.debug_utils_set_object_name(self.device.handle(), &debug_info)
                .expect("Failed to set debug object name");
        }
    }

    pub fn set_image_name(&self, image: &ImageWrapper, name: &str)
    {
        self.set_debug_name(vk::ObjectType::IMAGE, image.get().as_raw(), name);
    }

    pub fn allocate_memory(
        &mut self,
        name: &str,
        requirements: vk::MemoryRequirements,
        location: MemoryLocation,
        linear: bool) -> Allocation {

        unsafe {
            let alloc_name = name.to_owned() + "_allocation";
            self.allocator.allocate(&AllocationCreateDesc {
                name: &alloc_name,
                requirements,
                location,
                linear
            }).expect("Failed to allocate memory for Device resource")
        }
    }

    pub fn generate_handle(
        &mut self
    ) -> u64 {
        let new = self.handle_generator;
        self.handle_generator += 1;
        new
    }

    pub fn create_image(
        device: Rc<RefCell<DeviceWrapper>>,
        image_desc: &ImageCreateInfo,
        memory_location: MemoryLocation) -> DeviceResource {

        let device_image = {
            let new_handle = device.borrow_mut().generate_handle();
            let create_info = image_desc.get_create_info();
            let image = unsafe {
                device.borrow().get().create_image(create_info, None)
                    .expect("Failed to create image")
            };

            let memory_requirements = unsafe {
                device.borrow().get().get_image_memory_requirements(image)
            };

            let allocation = device.borrow_mut().allocate_memory(
                image_desc.get_name(),
                memory_requirements,
                memory_location,
                false);

            unsafe {
                device.borrow().get().bind_image_memory(
                    image,
                    allocation.memory(),
                    allocation.offset())
                    .expect("Failed to bind image to memory");
            }

            let image_view = device.borrow().create_image_view(
                image,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageViewCreateFlags::empty(),
                vk::ImageAspectFlags::COLOR,
                1);
            let image_wrapper = ImageWrapper::new(
                image,
                image_view,
                create_info.initial_layout,
                create_info.extent);

            device.borrow().set_image_name(&image_wrapper, image_desc.get_name());
            DeviceResource {
                allocation: Some(allocation),
                resource_type: Some(ResourceType::Image(image_wrapper)),
                handle: new_handle,
                device,
            }
        };

        device_image
    }

    pub fn wrap_image(
        device: Rc<RefCell<DeviceWrapper>>,
        image: vk::Image,
        format: vk::Format,
        image_aspect_flags: vk::ImageAspectFlags,
        mip_levels: u32,
        extent: vk::Extent3D
    ) -> DeviceResource {
        let new_handle = device.borrow_mut().generate_handle();

        let image_view = device.borrow().create_image_view(
            image,
            format,
            vk::ImageViewCreateFlags::empty(),
            image_aspect_flags,
            mip_levels);

        let image_wrapper = ImageWrapper::new(
            image,
            image_view,
            vk::ImageLayout::UNDEFINED,
            extent);

        DeviceResource {
            allocation: None,
            resource_type: Some(ResourceType::Image(image_wrapper)),
            handle: new_handle,
            device
        }
    }

    pub fn set_buffer_name(&self, buffer: &BufferWrapper, name: &str)
    {
        self.set_debug_name(vk::ObjectType::BUFFER, buffer.get().as_raw(), name);
    }

    pub fn create_buffer(
        device: Rc<RefCell<DeviceWrapper>>,
        buffer_desc: &BufferCreateInfo,
        memory_location: MemoryLocation) -> DeviceResource {

        let device_buffer = {
            let new_handle = device.borrow_mut().generate_handle();
            let create_info = buffer_desc.get_create_info();
            let buffer = unsafe {
                device.borrow().get().create_buffer(create_info, None)
                    .expect("Failed to create buffer")
            };

            let memory_requirements = unsafe {
                device.borrow().get().get_buffer_memory_requirements(buffer)
            };

            let allocation = device.borrow_mut().allocate_memory(
                buffer_desc.get_name(),
                memory_requirements,
                memory_location,
                true);

            unsafe {
                device.borrow().get().bind_buffer_memory(
                    buffer,
                    allocation.memory(),
                    allocation.offset())
                    .expect("Failed to bind buffer to memory");
            }

            let buffer_wrapper = BufferWrapper::new(buffer);
            device.borrow().set_buffer_name(&buffer_wrapper, buffer_desc.get_name());
            DeviceResource {
                allocation: Some(allocation),
                resource_type: Some(ResourceType::Buffer(buffer_wrapper)),
                handle: new_handle,
                device
            }
        };
        device_buffer
    }

    pub fn update_buffer<F>(&self, device_buffer: &DeviceResource, mut fill_callback: F)
        where F: FnMut(*mut c_void, u64) {
        let allocation = {
            match &device_buffer.allocation {
                Some(alloc) => { alloc },
                _ => {
                    panic!("Cannot update buffer with no allocation");
                }
            }
        };
        if let Some(resolved_resource) = &device_buffer.resource_type {
            if let ResourceType::Buffer(buffer) = &resolved_resource {
                if let Some(mapped) = allocation.mapped_ptr() {
                    // TODO: I believe this will occur if the memory is already host-visible?
                    fill_callback(mapped.as_ptr(), allocation.size());
                } else {
                    unsafe {
                        let mapped_memory = self.device.map_memory(
                            allocation.memory(),
                            allocation.offset(),
                            allocation.size(),
                            vk::MemoryMapFlags::empty())
                            .expect("Failed to map buffer");
                        fill_callback(mapped_memory, allocation.size());
                        self.device.unmap_memory(allocation.memory());
                    }
                }
            } else {
                panic!("Cannot update a non-buffer resource as a buffer");
            }
        } else {
            panic!("Cannot update an invalid buffer");
        }
    }
}
