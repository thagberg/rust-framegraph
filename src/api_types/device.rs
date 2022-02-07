use ash::vk;

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
    device: ash::Device
}

impl Drop for DeviceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

impl DeviceWrapper {
    pub fn new(device: ash::Device) -> DeviceWrapper {
        DeviceWrapper {
            device
        }
    }
    pub fn get(&self) -> &ash::Device {
        &self.device
    }
}
