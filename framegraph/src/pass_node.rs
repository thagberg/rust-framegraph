use std::fmt::{Debug, Formatter};
use ash::vk;
use crate::i_pass_node::PassNode;
use crate::resource::resource_manager::{ResourceHandle, ResolvedResourceMap};
use context::i_render_context::{RenderContext, CommandBuffer};
use crate::pipeline::{PipelineDescription};

type FillCallback<RC: RenderContext, CB: CommandBuffer> = dyn (
    Fn(
        &RC,
        &CB,
        &ResolvedResourceMap,
        &ResolvedResourceMap
    )
);

pub struct GraphicsPassNode<RC: RenderContext, CB: CommandBuffer> {
    pipeline_description: PipelineDescription,
    render_targets: Vec<ResourceHandle>,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    fill_callback: Box<FillCallback<RC, CB>>
}

#[derive(Default)]
pub struct PassNodeBuilder<RC: RenderContext, CB: CommandBuffer> {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<ResourceHandle>,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    fill_callback: Option<Box<FillCallback<RC, CB>>>
}

impl<RC, CB> PassNode<RC, CB> for GraphicsPassNode<RC, CB>
    where RC: RenderContext, CB: CommandBuffer {

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

   fn execute(
        &self,
        render_context: &mut RC,
        command_buffer: &CB,
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

impl<RC, CB> Debug for GraphicsPassNode<RC, CB>
    where RC: RenderContext, CB: CommandBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let pipeline_name = self.pipeline_description.get_name();
        f.debug_struct("PassNode")
            .field("Name", &pipeline_name.to_string())
            .finish()
    }
}

impl<RC, CB> GraphicsPassNode<RC, CB>
    where RC: RenderContext + std::default::Default, CB: CommandBuffer + std::default::Default {
    pub fn builder() -> PassNodeBuilder<RC, CB> {
        PassNodeBuilder {
            ..Default::default()
        }
    }

    pub fn get_pipeline_description(&self) -> &PipelineDescription { &self.pipeline_description }

}

impl<RC, CB> PassNodeBuilder<RC, CB>
    where RC: RenderContext, CB: CommandBuffer {
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

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback<RC, CB>>) -> Self
    {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn build(mut self) -> Result<GraphicsPassNode<RC, CB>, &'static str> {
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