use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::DeviceResource;
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::binding::{ResourceBinding};
use crate::pipeline::PipelineDescription;

pub trait PassNode {
    fn get_name(&self) -> &str;

    fn get_inputs(&self) -> &[ResourceBinding];

    fn get_inputs_mut(&mut self) -> &mut [ResourceBinding];

    fn get_outputs(&self) -> &[ResourceBinding];

    fn get_outputs_mut(&mut self) -> &mut [ResourceBinding];

    // fn get_rendertargets(&self) -> &[AttachmentReference];
    //
    // fn get_rendertargets_mut(&mut self) -> &mut [AttachmentReference];

    // fn get_copy_sources(&self) -> &[Rc<RefCell<DeviceResource>>];
    //
    // fn get_copy_dests(&self) -> &[Rc<RefCell<DeviceResource>>];

    fn get_pipeline_description(&self) -> &Option<PipelineDescription>;

    fn get_reads(&self) -> Vec<u64>;

    fn get_writes(&self) -> Vec<u64>;

    fn execute(
        &self,
        render_context: &mut VulkanRenderContext,
        command_buffer: &vk::CommandBuffer);
}