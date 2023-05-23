use crate::utility::debug::ValidationInfo;
use crate::utility::structures::DeviceExtension;
// use ash::vk_make_version;

use std::os::raw::c_char;

pub const APPLICATION_VERSION: u32 = ash::vk::make_api_version(0, 1, 0, 0);
pub const ENGINE_VERSION: u32 = ash::vk::make_api_version(0, 1, 0, 0);
pub const API_VERSION: u32 = ash::vk::make_api_version(0, 1, 0, 92);

pub const WINDOW_WIDTH: u32 = 800;
pub const WINDOW_HEIGHT: u32 = 600;
pub const VALIDATION: ValidationInfo = ValidationInfo {
    is_enable: true,
    required_validation_layers: ["VK_LAYER_KHRONOS_validation"],
};

impl DeviceExtension {
    pub fn get_extensions_raw_names(&self) -> [*const c_char; 1] {
        [
            // currently just enable the Swapchain extension.
            ash::extensions::khr::Swapchain::name().as_ptr(),
        ]
    }
}
