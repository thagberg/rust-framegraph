use std::fmt::{Debug, Formatter};
// use ash::extensions::ext::DebugUtils;
use ash::ext::debug_utils::Device;
use ash::vk::DebugUtilsMessengerEXT;

pub struct VulkanDebug {
    pub debug_utils: Device,
    pub debug_messenger: DebugUtilsMessengerEXT
}

impl Debug for VulkanDebug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanDebug")
            .finish()
    }
}
