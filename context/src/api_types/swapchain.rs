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

    fn _acquire_next_image_impl(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence
    ) -> (&ImageWrapper, u32)
    {
        let (image_index, is_sub_optimal) = unsafe
        {
            self.loader.acquire_next_image(
                self.swapchain,
                timeout,
                semaphore,
                fence)
            .expect("Failed to acquire next swpachain image")
        };
        (&self.images[image_index as usize], image_index)
    }

    pub fn acquire_next_image(
        &self,
        timeout: Option<u64>,
        semaphore: Option<vk::Semaphore>,
        fence: Option<vk::Fence>) -> (&ImageWrapper, u32)
    {
        let t = match timeout
        {
            Some(timeout) => timeout,
            _ => u64::MAX
        };
        let s = match semaphore
        {
            Some(semaphore) => semaphore,
            _ => vk::Semaphore::null()
        };
        let f = match fence
        {
            Some(fence) => fence,
            _ => vk::Fence::null()
        };
        self._acquire_next_image_impl(t, s, f)
    }
}

impl Drop for SwapchainWrapper {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_swapchain(self.swapchain, None);
        }
    }
}