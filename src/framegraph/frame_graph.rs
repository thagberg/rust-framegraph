use ash::vk;
use crate::{PassNode, RenderContext};
use crate::resource::resource_manager::{ResolvedResourceMap};
use crate::context::pipeline::{PipelineManager};

pub struct FrameGraph<'a> {
    nodes: Vec<&'a PassNode>,
    frame_started: bool,
    compiled: bool,
    pipeline_manager: PipelineManager
}

impl<'a> FrameGraph<'a> {
    pub fn new() -> FrameGraph<'a> {
        FrameGraph {
            nodes: vec![],
            frame_started: false,
            compiled: false,
            pipeline_manager: PipelineManager::new()
        }
    }

    pub fn start(&mut self) {
        assert!(!self.frame_started, "Can't start a frame that's already been started");
        self.frame_started = true;
    }

    pub fn add_node(&mut self, node: &'a PassNode) {
        assert!(self.frame_started, "Can't add PassNode before frame has been started");
        assert!(!self.compiled, "Can't add PassNode after frame has been compiled");
        self.nodes.push(node);
    }

    pub fn compile(&mut self) {
        assert!(self.frame_started, "Can't compile FrameGraph before it's been started");
        assert!(!self.compiled, "FrameGraph has already been compiled");
        self.compiled = true;
    }

    pub fn end(&mut self, render_context: &mut RenderContext, command_buffer: vk::CommandBuffer) {
        assert!(self.frame_started, "Can't end frame before it's been started");
        assert!(self.compiled, "Can't end frame before it's been compiled");
        let mut next = self.nodes.pop();
        while next.is_some() {
            let node = next.unwrap();
            let pipeline = self.pipeline_manager.create_pipeline(render_context, node.get_pipeline_description());
            // let mut resolved_inputs: Vec<ResolvedResource> = Vec::new();
            let mut resolved_inputs = ResolvedResourceMap::new();
            let mut resolved_outputs = ResolvedResourceMap::new();
            let inputs = node.get_inputs().as_ref();
            let outputs = node.get_outputs().as_ref();
            for input in inputs {
                let resolved = render_context.resolve_resource(
                    input);
                resolved_inputs.insert(input.clone(), resolved.clone());
            }
            for output in outputs {
                let resolved = render_context.resolve_resource(
                    output);
                resolved_outputs.insert(output.clone(), resolved.clone());
            }
            node.execute(
                render_context,
                command_buffer,
                &resolved_inputs,
                &resolved_outputs);
            next = self.nodes.pop();
        }
    }
}