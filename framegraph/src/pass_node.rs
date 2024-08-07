use std::fmt::{Debug};
use ash::vk;
use context::vulkan_render_context::VulkanRenderContext;

pub type FillCallback = dyn (
Fn(
    &VulkanRenderContext,
    &vk::CommandBuffer
)
);

pub trait PassNode {
    fn get_name(&self) -> &str;

    fn get_reads(&self) -> Vec<u64>;

    fn get_writes(&self) -> Vec<u64>;
}