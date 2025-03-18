use std::fmt::{Debug, Formatter};
use ash::vk::CommandBuffer;
use crate::binding::ResourceBinding;
use crate::pass_node::{FillCallback, PassNode};
use crate::pipeline::ComputePipelineDescription;

pub struct ComputePassNode<'d> {
    pub inputs: Vec<ResourceBinding<'d>>,
    pub outputs: Vec<ResourceBinding<'d>>,
    pub fill_callback: Box<FillCallback>,
    pub pipeline_description: ComputePipelineDescription,
    name: String
}

impl Debug for ComputePassNode<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputePassNode")
            .field("name", &self.name)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("pipeline description", &self.pipeline_description)
            .finish()
    }
}

impl<'d> ComputePassNode<'d> {
    pub fn builder(name: String) -> ComputePassNodeBuilder<'d> {
        ComputePassNodeBuilder {
            name,
            ..Default::default()
        }
    }

    pub fn execute(
        &self,
        command_buffer: CommandBuffer) {
        (self.fill_callback)(
            command_buffer);
    }
}

impl<'d> PassNode<'d> for ComputePassNode<'d> {
    fn get_name(&self) -> &str {
       &self.name
    }

    fn get_reads(&self) -> Vec<u64> {
        let mut reads: Vec<u64> = Vec::new();
        reads.reserve(self.inputs.len());
        for input in &self.inputs {
            reads.push(input.resource.lock().unwrap().get_handle());
        }
        reads
    }

    fn get_writes(&self) -> Vec<u64> {
        let mut writes: Vec<u64> = Vec::new();
        writes.reserve(self.outputs.len());
        for output in &self.outputs {
            writes.push(output.resource.lock().unwrap().get_handle());
        }
        writes
    }
}

#[derive(Default)]
pub struct ComputePassNodeBuilder<'d> {
    name: String,
    inputs: Vec<ResourceBinding<'d>>,
    outputs: Vec<ResourceBinding<'d>>,
    pipeline_description: Option<ComputePipelineDescription>,
    fill_callback: Option<Box<FillCallback>>,
}

impl<'d> ComputePassNodeBuilder<'d> {
    pub fn pipeline_description(mut self, pipeline_description: ComputePipelineDescription) -> Self {
        self.pipeline_description = Some(pipeline_description);
        self
    }

    pub fn input(mut self, input: ResourceBinding<'d>) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn output(mut self, output: ResourceBinding<'d>) -> Self {
        self.outputs.push(output);
        self
    }

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback>) -> Self {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn build(mut self) -> Result<ComputePassNode<'d>, &'static str> {
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