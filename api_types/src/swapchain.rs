use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use ash::prelude::VkResult;
use ash::vk;
use ash::khr::swapchain as ash_swapchain;
use crate::device::interface::DeviceInterface;
use crate::device::resource::DeviceResource;

#[derive(PartialEq, Eq)]
pub enum SwapchainStatus {
    Ok,
    Suboptimal,
    Outdated
}

pub struct NextImage<'a> {
    pub image: Option<Arc<Mutex<DeviceResource<'a>>>>,
    pub status: SwapchainStatus
}

pub struct SwapchainWrapper<'a> {
    device: &'a DeviceInterface,
    loader: ash_swapchain::Device,
    swapchain: vk::SwapchainKHR,
    images: Vec<Arc<Mutex<DeviceResource<'a>>>>,
    format: vk::Format,
    extent: vk::Extent2D,
    present_fences: Vec<vk::Fence>
}

impl Debug for SwapchainWrapper<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwapchainWrapper")
            .finish()
    }
}

impl<'a> SwapchainWrapper<'a> {
    pub fn new(
        device: &'a DeviceInterface,
        loader: ash_swapchain::Device,
        swapchain: vk::SwapchainKHR,
        images: Vec<Arc<Mutex<DeviceResource<'a>>>>,
        format: vk::Format,
        extent: vk::Extent2D,
        present_fences: Vec<vk::Fence>
    ) -> SwapchainWrapper<'a> {
        SwapchainWrapper {
            device,
            loader,
            swapchain,
            images,
            format,
            extent,
            present_fences
        }
    }

    pub fn get(&self) -> vk::SwapchainKHR { self.swapchain }

    pub fn get_images(&self) -> &'a Vec<Arc<Mutex<DeviceResource>>> { &self.images }

    pub fn get_format(&self) -> vk::Format { self.format }

    pub fn get_extent(&self) -> vk::Extent2D { self.extent }

    pub fn get_loader(&self) -> &ash_swapchain::Device { &self.loader }

    pub fn get_present_fence(&self, index: u32) -> vk::Fence {
        self.present_fences[index as usize].clone()
    }

    pub fn can_destroy(&self) -> bool {
        let mut can_destroy = true;

        unsafe {
            for fence in &self.present_fences {
                let fence_status = self.device.get_fence_status(*fence)
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
    ) -> NextImage<'a>
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
        fence: Option<vk::Fence>) -> NextImage<'a>
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

impl Drop for SwapchainWrapper<'_> {
    fn drop(&mut self) {
        unsafe {
            for fence in &self.present_fences {
                self.device.destroy_fence(*fence, None);
            }
            self.loader.destroy_swapchain(self.swapchain, None);
        }
    }
}