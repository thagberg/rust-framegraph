use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::DeviceResource;
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::binding::{ResourceBinding};
use crate::pipeline::PipelineDescription;

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

    fn execute(
        &self,
        render_context: &mut VulkanRenderContext,
        command_buffer: &vk::CommandBuffer);
}