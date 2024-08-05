use std::cell::RefCell;
use std::rc::Rc;
use ash::prelude::VkResult;
use ash::vk;
use crate::api_types::device::{DeviceResource, DeviceWrapper};

#[derive(PartialEq, Eq)]
pub enum SwapchainStatus {
    Ok,
    Suboptimal,
    Outdated
}

pub struct NextImage {
    pub image: Option<Rc<RefCell<DeviceResource>>>,
    pub status: SwapchainStatus
}

pub struct SwapchainWrapper {
    loader: ash::extensions::khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    images: Vec<Rc<RefCell<DeviceResource>>>,
    format: vk::Format,
    extent: vk::Extent2D,
    present_fences: Vec<vk::Fence>
}

impl SwapchainWrapper {
    pub fn new(
        loader: ash::extensions::khr::Swapchain,
        swapchain: vk::SwapchainKHR,
        images: Vec<Rc<RefCell<DeviceResource>>>,
        format: vk::Format,
        extent: vk::Extent2D,
        present_fences: Vec<vk::Fence>
    ) -> SwapchainWrapper {
        SwapchainWrapper {
            loader,
            swapchain,
            images,
            format,
            extent,
            present_fences
        }
    }

    pub fn get(&self) -> vk::SwapchainKHR { self.swapchain }

    pub fn get_images(&self) -> &Vec<Rc<RefCell<DeviceResource>>> { &self.images }

    pub fn get_format(&self) -> vk::Format { self.format }

    pub fn get_extent(&self) -> vk::Extent2D { self.extent }

    pub fn get_loader(&self) -> &ash::extensions::khr::Swapchain { &self.loader }

    pub fn get_present_fence(&self, index: u32) -> vk::Fence {
        self.present_fences[index as usize].clone()
    }

    pub fn can_destroy(&self, device: &DeviceWrapper) -> bool {
        let mut can_destroy = true;

        unsafe {
            for fence in &self.present_fences {
                let fence_status = device.get().get_fence_status(*fence)
                    .expect("Failed to get Present fence status");
                match fence_status {
                    true => {}
                    false => {can_destroy = false}
                }
            }
        }

        can_destroy
    }

    fn _acquire_next_image_impl(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence
    ) -> NextImage
    {
        let acquire_image = unsafe {
            self.loader.acquire_next_image(
                self.swapchain,
                timeout,
                semaphore,
                fence)
        };

        match acquire_image {
            Ok((image_index, is_sub_optimal)) => {
                let status = match is_sub_optimal {
                    true => {SwapchainStatus::Suboptimal}
                    false => {SwapchainStatus::Ok}
                };
                NextImage {
                    image: Some(self.images[image_index as usize].clone()),
                    status,
                }
            }
            Err(e) => {
                log::trace!(target: "swapchain", "Error when obtaining next swapchain image: {}", e);
                NextImage {
                    image: None,
                    status: SwapchainStatus::Outdated
                }
            }
        }
    }

    pub fn acquire_next_image(
        &self,
        timeout: Option<u64>,
        semaphore: Option<vk::Semaphore>,
        fence: Option<vk::Fence>) -> NextImage
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