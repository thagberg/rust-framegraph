use ash::vk;

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