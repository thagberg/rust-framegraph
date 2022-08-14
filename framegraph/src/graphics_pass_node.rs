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
        &VulkanCommandBuffer,
        &ResolvedResourceMap,
        &ResolvedResourceMap
    )
);

pub struct GraphicsPassNode {
    pipeline_description: PipelineDescription,
    render_targets: Vec<ResourceHandle>,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    fill_callback: Box<FillCallback>
}

#[derive(Default)]
pub struct PassNodeBuilder {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<ResourceHandle>,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    fill_callback: Option<Box<FillCallback>>
}

impl PassNode for GraphicsPassNode  {
    type RC = VulkanRenderContext;
    type CB = VulkanCommandBuffer;
    type PD = PipelineDescription;

    fn get_name(&self) -> &str {
        self.pipeline_description.get_name()
    }

   fn get_inputs(&self) -> &[ResourceHandle] {
        &self.inputs
    }

   fn get_outputs(&self) -> &[ResourceHandle] {
        &self.outputs
    }

   fn get_rendertargets(&self) -> &[ResourceHandle] { &self.render_targets }

    fn get_pipeline_description(&self) -> &Self::PD {
        &self.pipeline_description
    }

   fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB,
        resolved_inputs: &ResolvedResourceMap,
        resolved_outputs: &ResolvedResourceMap)
    {
        (self.fill_callback)(
            render_context,
            command_buffer,
            resolved_inputs,
            resolved_outputs);
    }

}

impl Debug for GraphicsPassNode  {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let pipeline_name = self.pipeline_description.get_name();
        f.debug_struct("PassNode")
            .field("Name", &pipeline_name.to_string())
            .finish()
    }
}

impl GraphicsPassNode  {
    pub fn builder() -> PassNodeBuilder {
        PassNodeBuilder {
            ..Default::default()
        }
    }

    pub fn get_pipeline_description(&self) -> &PipelineDescription { &self.pipeline_description }

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

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback>) -> Self
    {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn build(mut self) -> Result<GraphicsPassNode, &'static str> {
        assert!(self.pipeline_description.is_some(), "No pipeline set");
        assert!(self.fill_callback.is_some(), "No fill callback set");

        if self.pipeline_description.is_some() && self.fill_callback.is_some() {
            let rt_len = self.render_targets.len();
            let inputs_len = self.inputs.len();
            let outputs_len = self.outputs.len();
            Ok(GraphicsPassNode {
                pipeline_description: self.pipeline_description.unwrap(),
                render_targets: self.render_targets.into_iter().take(rt_len).collect(),
                inputs: self.inputs.into_iter().take(inputs_len).collect(),
                outputs: self.outputs.into_iter().take(outputs_len).collect(),
                fill_callback: self.fill_callback.take().unwrap()
            })
        } else {
            Err("PassNodeBuilder was incomplete before building")
        }
    }
}