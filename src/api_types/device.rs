use ash::vk;
use crate::api_types::image::ImageWrapper;

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

pub struct PhysicalDeviceWrapper {
    physical_device: vk::PhysicalDevice
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
    pub fn new(device: ash::Device, queue_family_indices: QueueFamilies) -> DeviceWrapper {
        DeviceWrapper {
            device,
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

    pub fn create_image(&self, create_info: &vk::ImageCreateInfo) -> (ImageWrapper, vk::MemoryRequirements)
    {

        let image_wrapper = {
            let image = unsafe {
                self.device.create_image(create_info, None)
                    .expect("Failed to create image")
            };
            let image_view = self.create_image_view(
                image,
                vk::Format::R8G8B8A8_SRGB,
                vk::ImageViewCreateFlags::empty(),
                vk::ImageAspectFlags::COLOR,
                1);
            ImageWrapper::new(image, image_view)
        };

        let memory_requirements = unsafe {
            self.device.get_image_memory_requirements(image_wrapper.image)
        };

        (image_wrapper, memory_requirements)
    }
}
