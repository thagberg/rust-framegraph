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

    pub fn get(&self) -> vk::SwapchainKHR { self.swapchain }

    pub fn get_images(&self) -> &Vec<ImageWrapper> { &self.images }

    pub fn get_format(&self) -> vk::Format { self.format }

    pub fn get_extent(&self) -> vk::Extent2D { self.extent }

    pub fn get_loader(&self) -> &ash::extensions::khr::Swapchain { &self.loader }
}

impl Drop for SwapchainWrapper {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_swapchain(self.swapchain, None);
        }
    }
}