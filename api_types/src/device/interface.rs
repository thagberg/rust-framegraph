use std::ffi::CString;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use ash::vk;
use ash::vk::{DebugUtilsObjectNameInfoEXT, Handle};
use crate::device::debug::VulkanDebug;
use crate::device::DeviceLifetime;
use crate::device::queue::QueueFamilies;
use crate::image::ImageWrapper;

pub struct DeviceInterface {
    device: ash::Device,
    queue_families: QueueFamilies,
    debug: Option<VulkanDebug>
}

impl Debug for DeviceInterface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceInterface")
            .finish()
    }
}

impl Drop for DeviceInterface {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

impl Deref for DeviceInterface {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl DeviceInterface {
    pub fn new(
        device: ash::Device,
        queue_families: QueueFamilies,
        debug: Option<VulkanDebug>) -> Self {
        DeviceInterface {
            device,
            queue_families,
            debug
        }
    }

    pub fn get(&self) -> &ash::Device { &self.device }

    pub fn get_queue_families(&self) -> &QueueFamilies { &self.queue_families }

    pub fn set_debug_name(&self, object_type: vk::ObjectType, handle: u64, name: &str)
    {
        let c_name = CString::new(name)
            .expect("Failed to create C-name for debug object");
        let debug_info = DebugUtilsObjectNameInfoEXT::builder()
            .object_type(object_type)
            .object_handle(handle)
            .object_name(&c_name)
            .build();
        unsafe {
            if let Some(debug) = &self.debug {
                debug.debug_utils.set_debug_utils_set_object_name(self.device.get().handle(), &debug_info)
                    .expect("Failed to set debug object name");
            }
        }
    }

    pub fn set_image_name(&self, image: &ImageWrapper, name: &str)
    {
        self.set_debug_name(vk::ObjectType::IMAGE, image.get().as_raw(), name);
    }
}