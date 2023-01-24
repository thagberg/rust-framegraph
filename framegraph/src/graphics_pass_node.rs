use std::fmt::{Debug, Formatter};
use ash::vk;
use ash::vk::{ImageMemoryBarrier, MemoryBarrier};
use context::api_types::renderpass::VulkanRenderPass;
use context::api_types::vulkan_command_buffer::VulkanCommandBuffer;
use crate::pass_node::{PassNode, ResolvedBindingMap};
use crate::binding::{ResourceBinding, ResolvedResourceBinding};
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResourceMap, VulkanResourceManager, ResolvedResource};
use context::render_context::{RenderContext, CommandBuffer};
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::pipeline::{PipelineDescription};

type FillCallback = dyn (
    Fn(
        &VulkanRenderContext,
        // &VulkanCommandBuffer,
        &vk::CommandBuffer,
        &ResolvedBindingMap,
        &ResolvedBindingMap,
        &ResolvedResourceMap,
        &ResolvedResourceMap
    )
);

pub struct GraphicsPassNode {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<AttachmentReference>,
    inputs: Vec<ResourceBinding>,
    outputs: Vec<ResourceBinding>,
    copy_sources: Vec<ResourceHandle>,
    copy_dests: Vec<ResourceHandle>,
    memory_barriers: Vec<vk::MemoryBarrier>,
    image_barriers: Vec<vk::ImageMemoryBarrier>,
    fill_callback: Box<FillCallback>,
    name: String
}

#[derive(Default)]
pub struct PassNodeBuilder {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<AttachmentReference>,
    inputs: Vec<ResourceBinding>,
    outputs: Vec<ResourceBinding>,
    copy_sources: Vec<ResourceHandle>,
    copy_dests: Vec<ResourceHandle>,
    memory_barriers: Vec<vk::MemoryBarrier>,
    image_barriers: Vec<vk::ImageMemoryBarrier>,
    fill_callback: Option<Box<FillCallback>>,
    name: String
}

impl PassNode for GraphicsPassNode  {
    type RC = VulkanRenderContext;
    // type CB = VulkanCommandBuffer;
    type CB = vk::CommandBuffer;
    type PD = PipelineDescription;

    fn get_name(&self) -> &str {
        &self.name
    }

   fn get_inputs(&self) -> &[ResourceBinding] {
        &self.inputs
    }

    fn get_inputs_mut(&mut self) -> &mut [ResourceBinding] {
        &mut self.inputs
    }

   fn get_outputs(&self) -> &[ResourceBinding] {
        &self.outputs
    }

    fn get_outputs_mut(&mut self) -> &mut [ResourceBinding] {
        &mut self.outputs
    }

   fn get_rendertargets(&self) -> &[AttachmentReference] { &self.render_targets }

    fn get_rendertargets_mut(&mut self) -> &mut [AttachmentReference] { &mut self.render_targets }

    fn get_copy_sources(&self) -> &[ResourceHandle] { &self.copy_sources }

    fn get_copy_dests(&self) -> &[ResourceHandle] { &self.copy_dests }

    fn get_pipeline_description(&self) -> &Option<Self::PD> {
        &self.pipeline_description
    }

    fn get_dependencies(&self) -> Vec<ResourceHandle> {
        let input_handles : Vec<ResourceHandle> = self.get_inputs().into_iter().map(|binding| {
            binding.handle
        }).collect();
        [&input_handles, self.get_copy_sources()].concat()
    }

    fn get_writes(&self) -> Vec<ResourceHandle> {
        let output_handles: Vec<ResourceHandle> = self.get_outputs().into_iter().map(|binding| {
            binding.handle
        }).collect();
        let rt_handles: Vec<ResourceHandle> = self.get_rendertargets().into_iter().map(|attachment_ref| {
            attachment_ref.handle
        }).collect();
        [&output_handles, &rt_handles, self.get_copy_dests()].concat()
    }

    fn get_memory_barriers(&self) -> &[vk::MemoryBarrier] {
        &self.memory_barriers
    }

    fn get_image_barriers(&self) -> &[ImageMemoryBarrier] {
        &self.image_barriers
    }

   fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB,
        resolved_inputs: &ResolvedBindingMap,
        resolved_outputs: &ResolvedBindingMap,
        resolved_copy_sources: &ResolvedResourceMap,
        resolved_copy_dests: &ResolvedResourceMap)
    {
        (self.fill_callback)(
            render_context,
            command_buffer,
            resolved_inputs,
            resolved_outputs,
            resolved_copy_sources,
            resolved_copy_dests);
    }

}

impl Debug for GraphicsPassNode  {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassNode")
            .field("Name", &self.name)
            .finish()
    }
}

impl GraphicsPassNode  {
    pub fn builder(name: String) -> PassNodeBuilder {
        PassNodeBuilder {
            name,
            ..Default::default()
        }
    }

    pub fn get_pipeline_description(&self) -> &Option<PipelineDescription> { &self.pipeline_description }

    pub fn add_memory_barrier(&mut self, memory_barrier: vk::MemoryBarrier) {
        self.memory_barriers.push(memory_barrier);
    }

    pub fn add_image_barrier(&mut self, image_barrier: vk::ImageMemoryBarrier) {
        self.image_barriers.push(image_barrier);
    }
}

impl PassNodeBuilder {
    pub fn pipeline_description(mut self, pipeline_description: PipelineDescription) -> Self {
        self.pipeline_description = Some(pipeline_description);
        self
    }

    pub fn read(mut self, input: ResourceBinding) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn write(mut self, output: ResourceBinding) -> Self {
        self.outputs.push(output);
        self
    }

    pub fn render_target(mut self, render_target: AttachmentReference) -> Self {
        self.render_targets.push(render_target);
        self
    }

    pub fn copy_src(mut self, copy_src: ResourceHandle) -> Self {
        self.copy_sources.push(copy_src);
        self
    }

    pub fn copy_dst(mut self, copy_dst: ResourceHandle) -> Self {
        self.copy_dests.push(copy_dst);
        self
    }

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback>) -> Self
    {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn memory_barrier(mut self, memory_barrier: vk::MemoryBarrier) -> Self {
        self.memory_barriers.push(memory_barrier);
        self
    }

    pub fn image_barrier(mut self, image_barrier: vk::ImageMemoryBarrier) -> Self {
        self.image_barriers.push(image_barrier);
        self
    }

    pub fn build(mut self) -> Result<GraphicsPassNode, &'static str> {
        assert!(self.fill_callback.is_some(), "No fill callback set");

        if self.fill_callback.is_some() {
            let rt_len = self.render_targets.len();
            let inputs_len = self.inputs.len();
            let outputs_len = self.outputs.len();
            let copy_sources_len = self.copy_sources.len();
            let copy_dests_len = self.copy_dests.len();
            let memory_barriers_len = self.memory_barriers.len();
            let image_barriers_len = self.image_barriers.len();
            Ok(GraphicsPassNode {
                name: self.name,
                pipeline_description: self.pipeline_description,
                render_targets: self.render_targets.into_iter().take(rt_len).collect(),
                inputs: self.inputs.into_iter().take(inputs_len).collect(),
                outputs: self.outputs.into_iter().take(outputs_len).collect(),
                copy_sources: self.copy_sources.into_iter().take(copy_sources_len).collect(),
                copy_dests: self.copy_dests.into_iter().take(copy_dests_len).collect(),
                memory_barriers: self.memory_barriers.into_iter().take(memory_barriers_len).collect(),
                image_barriers: self.image_barriers.into_iter().take(image_barriers_len).collect(),
                fill_callback: self.fill_callback.take().unwrap()
            })
        } else {
            Err("PassNodeBuilder was incomplete before building")
        }
    }
}