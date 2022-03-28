use ash::vk;
use crate::resource::resource_manager::{ResourceHandle};
use crate::context::render_context::{RenderContext};

type FillCallback = fn(&RenderContext, &vk::CommandBuffer);

pub struct PassNode {
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    renderpass: vk::RenderPass,
    inputs: Option<Vec<ResourceHandle>>,
    outputs: Option<Vec<ResourceHandle>>,
    // fill_callback: dyn FnMut()
    fill_callback: FillCallback
}

#[derive(Default)]
pub struct PassNodeBuilder {
    layout: Option<vk::PipelineLayout>,
    pipeline: Option<vk::Pipeline>,
    renderpass: Option<vk::RenderPass>,
    inputs: Option<Vec<ResourceHandle>>,
    outputs: Option<Vec<ResourceHandle>>,
    // fill_callback: Option<dyn FnMut()>
    fill_callback: Option<FillCallback>
}

impl PassNode {
    pub fn builder() -> PassNodeBuilder {
        PassNodeBuilder {
            ..Default::default()
        }
    }

    pub fn execute(&self) {
        (self.fill_callback)();
    }
}

impl PassNodeBuilder {
    pub fn layout(&mut self, layout: vk::PipelineLayout) -> &mut Self {
        self.layout = Some(layout);
        self
    }

    pub fn pipeline(&mut self, pipeline: vk::Pipeline) -> &mut Self {
        self.pipeline = Some(pipeline);
        self
    }

    pub fn renderpass(&mut self, renderpass: vk::RenderPass) -> &mut Self {
        self.renderpass = Some(renderpass);
        self
    }

    pub fn inputs(&mut self, inputs: Vec<ResourceHandle>) -> &mut Self {
        self.inputs = Some(inputs);
        self
    }

    pub fn outputs(&mut self, outputs: Vec<ResourceHandle>) -> &mut Self {
        self.outputs = Some(outputs);
        self
    }

    // pub fn fill_commands<F>(&mut self, mut fill_callback: F) -> &mut Self
    //     where F: FnMut()
    pub fn fill_commands(&mut self, fill_callback: FillCallback) -> &mut Self
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
            Ok(PassNode {
                layout: self.layout.unwrap(),
                pipeline: self.pipeline.unwrap(),
                renderpass: self.renderpass.unwrap(),
                inputs: self.inputs.take(),
                outputs: self.outputs.take(),
                fill_callback: self.fill_callback.unwrap()
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