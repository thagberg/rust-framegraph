use std::cell::RefCell;
use std::rc::Rc;
use ash::vk::CommandBuffer;
use context::api_types::device::DeviceResource;
use context::vulkan_render_context::VulkanRenderContext;
use crate::binding::ResourceBinding;
use crate::pass_node::{FillCallback, PassNode};
use crate::pipeline::ComputePipelineDescription;

pub struct ComputePassNode {
    pub inputs: Vec<ResourceBinding>,
    pub outputs: Vec<ResourceBinding>,
    pub fill_callback: Box<FillCallback>,
    pub pipeline_description: ComputePipelineDescription,
    name: String
}

impl ComputePassNode {
    pub fn builder(name: String) -> ComputePassNodeBuilder {
        ComputePassNodeBuilder {
            name,
            ..Default::default()
        }
    }
}

impl PassNode for ComputePassNode {
    fn get_name(&self) -> &str {
        todo!()
    }

    fn get_reads(&self) -> Vec<u64> {
        todo!()
    }

    fn get_writes(&self) -> Vec<u64> {
        todo!()
    }

    fn execute(&self, render_context: &mut VulkanRenderContext, command_buffer: &CommandBuffer) {
        todo!()
    }
}

#[derive(Default)]
pub struct ComputePassNodeBuilder {
    name: String,
    inputs: Vec<ResourceBinding>,
    outputs: Vec<ResourceBinding>,
    pipeline_description: Option<ComputePipelineDescription>,
    fill_callback: Option<Box<FillCallback>>,
}

impl ComputePassNodeBuilder {
    pub fn pipeline_description(mut self, pipeline_description: ComputePipelineDescription) -> Self {
        self.pipeline_description = Some(pipeline_description);
        self
    }

    pub fn input(mut self, input: ResourceBinding) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn output(mut self, output: ResourceBinding) -> Self {
        self.outputs.push(output);
        self
    }

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback>) -> Self {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn build(mut self) -> Result<ComputePassNode, &'static str> {
        let inputs_len = self.inputs.len();
        let outputs_len = self.outputs.len();

        if let Some(_) = &self.fill_callback {
            Ok(ComputePassNode {
                inputs: self.inputs.into_iter().take(inputs_len).collect(),
                outputs: self.outputs.into_iter().take(outputs_len).collect(),
                fill_callback: self.fill_callback.take().unwrap(),
                name: self.name,
                pipeline_description: self.pipeline_description
                    .expect("ComputePassNode requires a pipeline description")
            })
        } else {
            Err("ComputePassNodeBuilder was incomplete before building")
        }
    }
}