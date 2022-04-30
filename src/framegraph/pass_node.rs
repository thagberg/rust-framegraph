use std::collections::HashMap;
use ash::vk;
use crate::resource::resource_manager::{ResourceHandle, TransientResource, ResolvedResourceMap};
use crate::context::render_context::{RenderContext};
use crate::ResolvedResource;

type FillCallback = dyn (
    Fn(
        &RenderContext,
        vk::CommandBuffer,
        &ResolvedResourceMap,
        &ResolvedResourceMap,
        &ResolvedResourceMap
    )
);

pub struct PassNode {
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    renderpass: vk::RenderPass,
    inputs: Vec<ResourceHandle>,
    outputs: Vec<ResourceHandle>,
    creates: Vec<ResourceHandle>,
    fill_callback: Box<FillCallback>
}

#[derive(Default)]
pub struct PassNodeBuilder {
    layout: Option<vk::PipelineLayout>,
    pipeline: Option<vk::Pipeline>,
    renderpass: Option<vk::RenderPass>,
    inputs: Option<Vec<ResourceHandle>>,
    outputs: Option<Vec<ResourceHandle>>,
    creates: Option<Vec<ResourceHandle>>,
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
        resolved_outputs: &ResolvedResourceMap,
        resolved_creates: &ResolvedResourceMap)
    {
        (self.fill_callback)(
            render_context,
            command_buffer,
            resolved_inputs,
            resolved_outputs,
            resolved_creates);
    }

    pub fn get_pipeline(&self) -> vk::Pipeline { self.pipeline }

    pub fn get_renderpass(&self) -> vk::RenderPass { self.renderpass }

    pub fn get_inputs(&self) -> &[ResourceHandle] {
        &self.inputs
    }

    pub fn get_outputs(&self) -> &[ResourceHandle] {
        &self.outputs
    }

    pub fn get_creates(&self) -> &[ResourceHandle] { &self.creates }
}

impl PassNodeBuilder {
    pub fn layout(mut self, layout: vk::PipelineLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    pub fn pipeline(mut self, pipeline: vk::Pipeline) -> Self {
        self.pipeline = Some(pipeline);
        self
    }

    pub fn renderpass(mut self, renderpass: vk::RenderPass) -> Self {
        self.renderpass = Some(renderpass);
        self
    }

    pub fn read(mut self, inputs: Vec<ResourceHandle>) -> Self {
        self.inputs = Some(inputs);
        self
    }

    pub fn write(mut self, outputs: Vec<ResourceHandle>) -> Self {
        self.outputs = Some(outputs);
        self
    }

    pub fn create(mut self, creates: Vec<ResourceHandle>) -> Self {
        self.creates = Some(creates);
        self
    }

    pub fn fill_commands(&mut self, fill_callback: Box<FillCallback>) -> &mut Self
    {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn build(&mut self) -> Result<PassNode, &'static str> {
        assert!(self.layout.is_some(), "No layout set");
        assert!(self.pipeline.is_some(), "No pipeline set");
        assert!(self.renderpass.is_some(), "No renderpass set");
        assert!(self.fill_callback.is_some(), "No fill callback set");

        if self.layout.is_some() && self.pipeline.is_some() && self.renderpass.is_some() {
            let inputs = match &self.inputs {
                Some(i) => { self.inputs.take().unwrap()},
                _ => {Vec::new()}
            };

            let outputs = match &self.outputs {
                Some(o) => { self.outputs.take().unwrap()},
                _ => {Vec::new()}
            };

            let creates = match &self.creates {
                Some(o) => { self.creates.take().unwrap()},
                _ => {Vec::new()}
            };

            Ok(PassNode {
                layout: self.layout.unwrap(),
                pipeline: self.pipeline.unwrap(),
                renderpass: self.renderpass.unwrap(),
                inputs: inputs,
                outputs: outputs,
                creates: creates,
                fill_callback: self.fill_callback.take().unwrap()
            })
        } else {
            Err("PassNodeBuilder was incomplete before building")
        }
    }
}

/*
subpass requirements
----------------------
color attachments
depth-stencil attachment
input attachments
resolve attachments     ---|
preserve attachments    ---|-- can probably ignore these

src subpass
dst subpass
src stage
dst stage
src access
dst access
-- should probably compute all of these while compiling the framegraph?

pipeline requirements
---------------------
shaders -- shader modules / shader stages
vertex input state
vertex assembly state
rasterization state
multisample state
stencil state
depth state
color blend states
descriptor set layouts

 */