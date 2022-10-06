use std::ffi::{CStr, CString};
use ash::vk;
use ash::extensions::ext::DebugUtils;
use ash::vk::{DebugUtilsObjectNameInfoEXT, Handle};
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
    queue_family_indices: QueueFamilies
}

impl Drop for DeviceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

impl DeviceWrapper {
    pub fn new(device: ash::Device, debug_utils: DebugUtils, queue_family_indices: QueueFamilies) -> DeviceWrapper {
        DeviceWrapper {
            device,
            debug_utils,
            queue_family_indices
        }
    }
    pub fn get(&self) -> &ash::Device {
        &self.device
    }
    pub fn get_queue_family_indices(&self) -> &QueueFamilies { &self.queue_family_indices }

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

    pub fn create_image(
        &self,
        create_info: &ImageCreateInfo,
        allocate_callback: &mut dyn FnMut(vk::MemoryRequirements) -> (vk::DeviceMemory, vk::DeviceSize)) -> ImageWrapper
    {

        let image_wrapper = {
            let image = unsafe {
                self.device.create_image(create_info.get_create_info(), None)
                    .expect("Failed to create image")
            };

            let memory_requirements = unsafe {
                self.device.get_image_memory_requirements(image)
            };

            let (memory, offset) = allocate_callback(memory_requirements);
            unsafe {
                self.device.bind_image_memory(image, memory, offset)
                    .expect("Failed to bind image to memory");
            }

            let image_view = self.create_image_view(
                image,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageViewCreateFlags::empty(),
                vk::ImageAspectFlags::COLOR,
                1);
            ImageWrapper::new(image, image_view)
        };

        self.set_image_name(&image_wrapper, create_info.get_name());

        image_wrapper
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
