use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use ash::vk;
use context::api_types::device::{DeviceFramebuffer, DeviceResource};
use crate::pass_node::{PassNode};
use crate::binding::{ResourceBinding};
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::pipeline::{PipelineDescription};

type FillCallback = dyn (
    Fn(
        &VulkanRenderContext,
        &vk::CommandBuffer
    )
);

pub struct GraphicsPassNode {
    pub pipeline_description: Option<PipelineDescription>,
    pub render_targets: Vec<AttachmentReference>,
    pub inputs: Vec<ResourceBinding>,
    pub outputs: Vec<ResourceBinding>,
    pub copy_sources: Vec<Rc<RefCell<DeviceResource>>>,
    pub copy_dests: Vec<Rc<RefCell<DeviceResource>>>,
    pub tagged_resources: Vec<Rc<RefCell<DeviceResource>>>,
    pub framebuffer: Option<DeviceFramebuffer>,
    pub fill_callback: Box<FillCallback>,
    name: String
}

#[derive(Default)]
pub struct PassNodeBuilder {
    pipeline_description: Option<PipelineDescription>,
    render_targets: Vec<AttachmentReference>,
    inputs: Vec<ResourceBinding>,
    outputs: Vec<ResourceBinding>,
    copy_sources: Vec<Rc<RefCell<DeviceResource>>>,
    copy_dests: Vec<Rc<RefCell<DeviceResource>>>,
    tagged_resources: Vec<Rc<RefCell<DeviceResource>>>,
    fill_callback: Option<Box<FillCallback>>,
    name: String
}

impl PassNode for GraphicsPassNode  {
    type RC = VulkanRenderContext;
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

    fn get_copy_sources(&self) -> &[Rc<RefCell<DeviceResource>>] { &self.copy_sources }

    fn get_copy_dests(&self) -> &[Rc<RefCell<DeviceResource>>] { &self.copy_dests }

    fn get_pipeline_description(&self) -> &Option<Self::PD> {
        &self.pipeline_description
    }

   fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB)
    {
        (self.fill_callback)(
            render_context,
            command_buffer);
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

    // pub fn set_framebuffer(&mut self, framebuffer: DeviceFramebuffer) {
    pub fn set_framebuffer(passnode: &mut Self, framebuffer: DeviceFramebuffer) {
        passnode.framebuffer = Some(framebuffer);
    }

    pub fn get_framebuffer(&self) -> vk::Framebuffer {
        if let Some(fb) = &self.framebuffer {
            fb.get_framebuffer()
        } else {
            panic!("No framebuffer was set on this pass");
        }
    }
}

impl PassNodeBuilder {
    pub fn pipeline_description(mut self, pipeline_description: PipelineDescription) -> Self {
        self.pipeline_description = Some(pipeline_description);
        self
    }

    pub fn tag(mut self, tagged_resource: Rc<RefCell<DeviceResource>>) -> Self {
        self.tagged_resources.push(tagged_resource);
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

    pub fn copy_src(mut self, copy_src: Rc<RefCell<DeviceResource>>) -> Self {
        self.copy_sources.push(copy_src);
        self
    }

    pub fn copy_dst(mut self, copy_dst: Rc<RefCell<DeviceResource>>) -> Self {
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
            let tagged_resources_len = self.tagged_resources.len();
            Ok(GraphicsPassNode {
                name: self.name,
                pipeline_description: self.pipeline_description,
                render_targets: self.render_targets.into_iter().take(rt_len).collect(),
                inputs: self.inputs.into_iter().take(inputs_len).collect(),
                outputs: self.outputs.into_iter().take(outputs_len).collect(),
                copy_sources: self.copy_sources.into_iter().take(copy_sources_len).collect(),
                copy_dests: self.copy_dests.into_iter().take(copy_dests_len).collect(),
                tagged_resources: self.tagged_resources.into_iter().take(tagged_resources_len).collect(),
                framebuffer: None,
                fill_callback: self.fill_callback.take().unwrap()
            })
        } else {
            Err("PassNodeBuilder was incomplete before building")
        }
    }
}