use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use api_types::device;
use api_types::device::interface::DeviceInterface;
use ash::vk;
use api_types::device::resource::DeviceResource;
use api_types::framebuffer::DeviceFramebuffer;
use crate::pass_node::{PassNode, FillCallback};
use crate::binding::{ResourceBinding};
use crate::attachment::AttachmentReference;
use crate::pipeline::{PipelineDescription};

pub struct GraphicsPassNode<'device> {
    pub pipeline_description: Option<Arc<PipelineDescription<'device>>>,
    pub render_targets: Vec<AttachmentReference<'device>>,
    pub depth_target: Option<AttachmentReference<'device>>,
    pub inputs: Vec<ResourceBinding<'device>>,
    pub outputs: Vec<ResourceBinding<'device>>,
    pub tagged_resources: Vec<Arc<Mutex<DeviceResource<'device>>>>,
    pub framebuffer: Option<DeviceFramebuffer<'device>>,
    pub viewport: Option<vk::Viewport>,
    pub scissor: Option<vk::Rect2D>,
    pub fill_callback: Box<FillCallback>,
    name: String
}

#[derive(Default)]
pub struct PassNodeBuilder<'device> {
    pipeline_description: Option<Arc<PipelineDescription<'device>>>,
    render_targets: Vec<AttachmentReference<'device>>,
    depth_target: Option<AttachmentReference<'device>>,
    inputs: Vec<ResourceBinding<'device>>,
    outputs: Vec<ResourceBinding<'device>>,
    tagged_resources: Vec<Arc<Mutex<DeviceResource<'device>>>>,
    fill_callback: Option<Box<FillCallback>>,
    viewport: Option<vk::Viewport>,
    scissor: Option<vk::Rect2D>,
    name: String
}

impl<'d> PassNode<'d> for GraphicsPassNode<'d>  {

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_reads(&self) -> Vec<u64> {
        let mut reads: Vec<u64> = Vec::new();
        reads.reserve(self.inputs.len() + self.render_targets.len());
        for input in &self.inputs {
           reads.push(input.resource.lock().unwrap().get_handle());
        }
        // color and depth targets also likely depend on previous writes
        for rt in &self.render_targets {
            reads.push(rt.resource_image.lock().unwrap().get_handle());
        }
        if let Some(dt) = &self.depth_target {
            reads.push(dt.resource_image.lock().unwrap().get_handle());
        }

        reads
    }

    fn get_writes(&self) -> Vec<u64> {
        let mut writes: Vec<u64> = Vec::new();
        for output in &self.outputs {
            writes.push(output.resource.lock().unwrap().get_handle());
        }
        for rt in &self.render_targets {
            writes.push(rt.resource_image.lock().unwrap().get_handle());
        }
        if let Some(dt) = &self.depth_target {
            writes.push(dt.resource_image.lock().unwrap().get_handle());
        }

        writes
    }

}

impl Debug for GraphicsPassNode<'_>  {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassNode")
            .field("Name", &self.name)
            .finish()
    }
}

impl<'device> GraphicsPassNode<'device>  {
    pub fn builder(name: String) -> PassNodeBuilder<'device> {
        PassNodeBuilder {
            name,
            ..Default::default()
        }
    }

    pub fn get_pipeline_description(&self) -> &Option<Arc<PipelineDescription<'device>>> { &self.pipeline_description }

    // pub fn set_framebuffer(&mut self, framebuffer: DeviceFramebuffer) {
    pub fn set_framebuffer(passnode: &mut Self, framebuffer: DeviceFramebuffer<'device>) {
        passnode.framebuffer = Some(framebuffer);
    }

    pub fn get_framebuffer(&self) -> vk::Framebuffer {
        if let Some(fb) = &self.framebuffer {
            fb.get_framebuffer()
        } else {
            panic!("No framebuffer was set on this pass");
        }
    }

    pub fn get_inputs(&self) -> &[ResourceBinding<'device>] {
        &self.inputs
    }

    pub fn get_inputs_mut(&mut self) -> &mut [ResourceBinding<'device>] {
        &mut self.inputs
    }

    pub fn get_outputs(&self) -> &[ResourceBinding<'device>] {
        &self.outputs
    }

    pub fn get_outputs_mut(&mut self) -> &mut [ResourceBinding<'device>] {
        &mut self.outputs
    }

    pub fn get_rendertargets_mut(&mut self) -> &mut [AttachmentReference<'device>] {
        &mut self.render_targets
    }

    pub fn get_depth_mut(&mut self) -> &mut Option<AttachmentReference<'device>> {
        &mut self.depth_target
    }

    pub fn execute(
        &self,
        device: &DeviceInterface,
        command_buffer: vk::CommandBuffer)
    {
        (self.fill_callback)(
            device,
            command_buffer);
    }

}

impl<'device> PassNodeBuilder<'device> {
    pub fn pipeline_description(mut self, pipeline_description: Arc<PipelineDescription<'device>>) -> Self {
        self.pipeline_description = Some(pipeline_description);
        self
    }

    pub fn tag(mut self, tagged_resource: Arc<Mutex<DeviceResource<'device>>>) -> Self {
        self.tagged_resources.push(tagged_resource);
        self
    }

    pub fn read(mut self, input: ResourceBinding<'device>) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn write(mut self, output: ResourceBinding<'device>) -> Self {
        self.outputs.push(output);
        self
    }

    pub fn render_target(mut self, render_target: AttachmentReference<'device>) -> Self {
        self.render_targets.push(render_target);
        self
    }

    pub fn depth_target(mut self, depth_target: AttachmentReference<'device>) -> Self {
        self.depth_target = Some(depth_target);
        self
    }

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback>) -> Self
    {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn viewport(mut self, viewport: vk::Viewport) -> Self
    {
        self.viewport = Some(viewport);
        self
    }

    pub fn scissor(mut self, scissor: vk::Rect2D) -> Self
    {
        self.scissor = Some(scissor);
        self
    }

    pub fn build(mut self) -> Result<GraphicsPassNode<'device>, &'static str> {
        assert!(self.fill_callback.is_some(), "No fill callback set");

        if self.fill_callback.is_some() {
            let rt_len = self.render_targets.len();
            let inputs_len = self.inputs.len();
            let outputs_len = self.outputs.len();
            let tagged_resources_len = self.tagged_resources.len();
            Ok(GraphicsPassNode {
                name: self.name,
                pipeline_description: self.pipeline_description,
                render_targets: self.render_targets.into_iter().take(rt_len).collect(),
                depth_target: self.depth_target,
                inputs: self.inputs.into_iter().take(inputs_len).collect(),
                outputs: self.outputs.into_iter().take(outputs_len).collect(),
                tagged_resources: self.tagged_resources.into_iter().take(tagged_resources_len).collect(),
                framebuffer: None,
                viewport: self.viewport,
                scissor: self.scissor,
                fill_callback: self.fill_callback.take().unwrap()
            })
        } else {
            Err("PassNodeBuilder was incomplete before building")
        }
    }
}