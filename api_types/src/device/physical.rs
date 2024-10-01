use ash::vk;

#[derive(Copy, Clone, Debug)]
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

