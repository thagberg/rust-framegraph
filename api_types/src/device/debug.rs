use std::ffi::CString;
use std::fmt::{Debug, Formatter};
// use ash::extensions::ext::DebugUtils;
use ash::vk;
use ash::vk::{DebugUtilsLabelEXT, DebugUtilsMessengerEXT, DebugUtilsObjectNameInfoEXT};

pub struct VulkanDebug {
    pub debug_utils: ash::ext::debug_utils::Instance,
    device_utils: Option<ash::ext::debug_utils::Device>,
    pub debug_messenger: DebugUtilsMessengerEXT
}

impl Debug for VulkanDebug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanDebug")
            .finish()
    }
}

impl VulkanDebug {

    pub fn new(
        debug_utils: ash::ext::debug_utils::Instance,
        debug_messenger: DebugUtilsMessengerEXT) -> Self {
        VulkanDebug {
            debug_utils,
            device_utils: None,
            debug_messenger,
        }
    }

    pub fn create_device_utils(&mut self, instance: &ash::Instance, device: &ash::Device) {
        let device_utils = ash::ext::debug_utils::Device::new(instance, device);
        self.device_utils = Some(device_utils);
    }
    pub fn set_object_name(&self, debug_info: &DebugUtilsObjectNameInfoEXT) {

        if let Some(device_utils) = &self.device_utils {
            unsafe {
                device_utils.set_debug_utils_object_name(debug_info)
                    .expect("failed to set debug object name");
            }
        }
    }

    pub fn begin_label(&self, label: &str, command_buffer: vk::CommandBuffer) {

        let c_label = CString::new(label)
            .expect("Failed to create C-string for debug label");
        let debug_label = DebugUtilsLabelEXT::default()
            .label_name(&c_label);
        if let Some(device_utils) = &self.device_utils {
            unsafe {
                device_utils.cmd_begin_debug_utils_label(command_buffer, &debug_label);
            }
        }
    }

    pub fn end_label(&self, command_buffer: vk::CommandBuffer) {
        if let Some(device_utils) = &self.device_utils {
            unsafe {
                device_utils.cmd_end_debug_utils_label(command_buffer);
            }
        }
    }

}