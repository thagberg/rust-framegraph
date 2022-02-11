use ash::extensions::khr::Swapchain;
use ash::vk;
use crate::api_types::image::ImageWrapper;

pub struct SwapchainWrapper {
    loader: ash::extensions::khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    images: Vec<ImageWrapper>,
    format: vk::Format,
    extent: vk::Extent2D
}

impl SwapchainWrapper {
    pub fn new(
        loader: ash::extensions::khr::Swapchain,
        swapchain: vk::SwapchainKHR,
        images: Vec<ImageWrapper>,
        format: vk::Format,
        extent: vk::Extent2D
    ) -> SwapchainWrapper {
        SwapchainWrapper {
            loader,
            swapchain,
            images,
            format,
            extent
        }
    }
}