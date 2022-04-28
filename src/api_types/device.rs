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

    pub fn create_image(&self, create_info: &vk::ImageCreateInfo) -> (ImageWrapper, vk::MemoryRequirements)
    {

        let mut image = ImageWrapper::new(unsafe {
            self.device.create_image(create_info, None)
                .expect("Failed to create image")
        });

        let memory_requirements = unsafe {
            self.device.get_image_memory_requirements(image.image)
        };

        let image_view = unsafe {
            self.device.create_image_view(
                &vk::ImageViewCreateInfo::builder()
                    .format(vk::Format::R8G8B8A8_SRGB)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .components(vk::ComponentMapping::builder()
                        .r(vk::ComponentSwizzle::IDENTITY)
                        .g(vk::ComponentSwizzle::IDENTITY)
                        .b(vk::ComponentSwizzle::IDENTITY)
                        .a(vk::ComponentSwizzle::IDENTITY)
                        .build())
                    .subresource_range(vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build())
                    .image(image.image),
            None)
                .expect("Failed to create image view")
        };
        image.view = Some(image_view);

        (image, memory_requirements)
    }
}
