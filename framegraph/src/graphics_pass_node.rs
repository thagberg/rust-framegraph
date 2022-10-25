use std::fmt::{Debug, Formatter};
use ash::vk;
use context::api_types::renderpass::VulkanRenderPass;
use context::api_types::vulkan_command_buffer::VulkanCommandBuffer;
use crate::pass_node::PassNode;
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResourceMap, VulkanResourceManager};
use context::render_context::{RenderContext, CommandBuffer};
use context::vulkan_render_context::VulkanRenderContext;
use crate::pipeline::{PipelineDescription};

type FillCallback = dyn (
    Fn(
        &VulkanRenderContext,
        // &VulkanCommandBuffer,
        &vk::CommandBuffer,
        &ResolvedResourceMap,
        &ResolvedResourceMap,
        &ResolvedResourceMap,
        &ResolvedResourceMap
    )
);

pub struct GraphicsPassNode {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<ResourceHandle>,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    copy_sources: Vec<ResourceHandle>,
    copy_dests: Vec<ResourceHandle>,
    fill_callback: Box<FillCallback>,
    name: String
}

#[derive(Default)]
pub struct PassNodeBuilder {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<ResourceHandle>,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    copy_sources: Vec<ResourceHandle>,
    copy_dests: Vec<ResourceHandle>,
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

   fn get_inputs(&self) -> &[ResourceHandle] {
        &self.inputs
    }

   fn get_outputs(&self) -> &[ResourceHandle] {
        &self.outputs
    }

   fn get_rendertargets(&self) -> &[ResourceHandle] { &self.render_targets }

    fn get_copy_sources(&self) -> &[ResourceHandle] { &self.copy_sources }

    fn get_copy_dests(&self) -> &[ResourceHandle] { &self.copy_dests }

    fn get_pipeline_description(&self) -> &Option<Self::PD> {
        &self.pipeline_description
    }

    fn get_dependencies(&self) -> Vec<ResourceHandle> {
        [self.get_inputs(), self.get_copy_sources()].concat()
    }

    fn get_writes(&self) -> Vec<ResourceHandle> {
        [self.get_outputs(), self.get_rendertargets(), self.get_copy_dests()].concat()
    }

   fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB,
        resolved_inputs: &ResolvedResourceMap,
        resolved_outputs: &ResolvedResourceMap,
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

}

impl PassNodeBuilder {
    pub fn pipeline_description(mut self, pipeline_description: PipelineDescription) -> Self {
        self.pipeline_description = Some(pipeline_description);
        self
    }

    pub fn read(mut self, input: ResourceHandle) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn write(mut self, output: ResourceHandle) -> Self {
        self.outputs.push(output);
        self
    }

    pub fn render_target(mut self, render_target: ResourceHandle) -> Self {
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

    pub fn build(mut self) -> Result<GraphicsPassNode, &'static str> {
        assert!(self.fill_callback.is_some(), "No fill callback set");

        if self.fill_callback.is_some() {
            let rt_len = self.render_targets.len();
            let inputs_len = self.inputs.len();
            let outputs_len = self.outputs.len();
            let copy_sources_len = self.copy_sources.len();
            let copy_dests_len = self.copy_dests.len();
            Ok(GraphicsPassNode {
                name: self.name,
                pipeline_description: self.pipeline_description,
                render_targets: self.render_targets.into_iter().take(rt_len).collect(),
                inputs: self.inputs.into_iter().take(inputs_len).collect(),
                outputs: self.outputs.into_iter().take(outputs_len).collect(),
                copy_sources: self.copy_sources.into_iter().take(copy_sources_len).collect(),
                copy_dests: self.copy_dests.into_iter().take(copy_dests_len).collect(),
                fill_callback: self.fill_callback.take().unwrap()
            })
        } else {
            Err("PassNodeBuilder was incomplete before building")
        }
    }
}