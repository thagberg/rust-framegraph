use std::cell::RefCell;
use std::ffi::{CStr, CString};
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
    allocator: Allocator
}

#[derive(Clone)]
pub enum ResourceType {
    Buffer(BufferWrapper),
    Image(ImageWrapper)
}

pub struct AllocatedResource {
    pub allocation: Allocation,
    pub resource_type: ResourceType
}

pub struct DeviceResource {
    pub resource: Option<AllocatedResource>,

    device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DeviceResource {
    fn drop(&mut self) {
        if let Some(resource) = &mut self.resource {
            let allocation = std::mem::replace(&mut resource.allocation, Allocation::default());
            match &resource.resource_type {
                ResourceType::Buffer(buffer) => {
                    panic!("Not implemented for buffers yet");
                },
                ResourceType::Image(image) => {
                    self.device.borrow_mut().destroy_image(image, allocation);
                }
            }
        }
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
            allocator
        }
    }
    pub fn get(&self) -> &ash::Device {
        &self.device
    }
    pub fn get_queue_family_indices(&self) -> &QueueFamilies { &self.queue_family_indices }

    pub fn destroy_image(&mut self, image: &ImageWrapper, allocation: Allocation) {
        unsafe {
            self.device.destroy_image_view(image.view, None);
            self.device.destroy_image(image.image, None);
            self.allocator.free(allocation)
                .expect("Failed to free image allocation");
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

    pub fn create_image(
        device: Rc<RefCell<DeviceWrapper>>,
        image_desc: &ImageCreateInfo,
        memory_location: MemoryLocation) -> DeviceResource {

        let device_image = {
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
                resource: Some(AllocatedResource {
                    allocation,
                    resource_type: ResourceType::Image(image_wrapper)
                }),
                device
            }
        };

        device_image
    }

    pub fn set_buffer_name(&self, buffer: &BufferWrapper, name: &str)
    {
        self.set_debug_name(vk::ObjectType::BUFFER, buffer.get().as_raw(), name);
    }

    pub fn create_buffer(
        &self,
        create_info: &BufferCreateInfo,
        allocate_callback: &mut dyn FnMut(vk::MemoryRequirements) -> (vk::DeviceMemory, vk::DeviceSize)) -> BufferWrapper {

        let buffer = unsafe {
            self.device.create_buffer(create_info.get_create_info(), None)
                .expect("Failed to create uniform buffer")
        };
        let requirements = unsafe {
            self.device.get_buffer_memory_requirements(buffer)
        };

        let (memory, offset) = allocate_callback(requirements);
        unsafe {
            self.device.bind_buffer_memory( buffer, memory, offset)
                .expect("Failed to bind buffer to memory");
        }

        let buffer_wrapper = BufferWrapper::new(buffer);

        self.set_buffer_name(&buffer_wrapper, create_info.get_name());

        buffer_wrapper
    }
}
