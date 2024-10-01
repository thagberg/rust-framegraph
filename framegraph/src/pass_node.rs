use std::fmt::{Debug};
use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::DeviceWrapper;
use context::vulkan_render_context::VulkanRenderContext;

pub type FillCallback = dyn (
Fn(
    Arc<Mutex<DeviceWrapper>>,
    vk::CommandBuffer
)
);

pub trait PassNode {
    fn get_name(&self) -> &str;

    fn get_reads(&self) -> Vec<u64>;

    fn get_writes(&self) -> Vec<u64>;
}