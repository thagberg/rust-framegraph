use ash::vk;
use crate::resource::resource_manager::{ResourceHandle, ResolvedResourceMap};
use context::render_context::{RenderContext};
use crate::pipeline::{PipelineDescription};

type FillCallback = dyn (
    Fn(
        &RenderContext,
        vk::CommandBuffer,
        &ResolvedResourceMap,
        &ResolvedResourceMap
    )
);

pub struct PassNode {
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

impl PassNode {
    pub fn builder() -> PassNodeBuilder {
        PassNodeBuilder {
            ..Default::default()
        }
    }

    pub fn execute(
        &self,
        render_context: &RenderContext,
        command_buffer: vk::CommandBuffer,
        resolved_inputs: &ResolvedResourceMap,
        resolved_outputs: &ResolvedResourceMap)
    {
        (self.fill_callback)(
            render_context,
            command_buffer,
            resolved_inputs,
            resolved_outputs);
    }

    pub fn get_pipeline_description(&self) -> &PipelineDescription { &self.pipeline_description }

    pub fn get_inputs(&self) -> &[ResourceHandle] {
        &self.inputs
    }

    pub fn get_outputs(&self) -> &[ResourceHandle] {
        &self.outputs
    }

    pub fn get_rendertargets(&self) -> &[ResourceHandle] { &self.render_targets }
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

    pub fn build(mut self) -> Result<PassNode, &'static str> {
        assert!(self.pipeline_description.is_some(), "No pipeline set");
        assert!(self.fill_callback.is_some(), "No fill callback set");

        if self.pipeline_description.is_some() && self.fill_callback.is_some() {
            let rt_len = self.render_targets.len();
            let inputs_len = self.inputs.len();
            let outputs_len = self.outputs.len();
            Ok(PassNode {
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