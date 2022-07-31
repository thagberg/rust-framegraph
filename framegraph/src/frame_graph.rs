extern crate petgraph;
use petgraph::{graph, stable_graph, Direction};
extern crate multimap;
use multimap::MultiMap;

extern crate context;
use context::render_context::RenderContext;

use ash::vk;
use crate::pass_node::{PassNode};
use crate::pipeline::{PipelineManager};
use crate::resource::resource_manager::{ResolvedResourceMap, ResourceHandle, ResourceManager};

use std::collections::HashMap;


pub struct FrameGraph<'a> {
    // nodes: Dag::<&'a PassNode, u32>,
    // nodes: Vec<&'a PassNode>,
    // nodes: graph::DiGraph<&'a PassNode, u32>,
    nodes: stable_graph::StableDiGraph<PassNode, u32>,
    // sorted_nodes: Vec<&'a PassNode>,
    frame_started: bool,
    compiled: bool,
    pipeline_manager: PipelineManager,
    resource_manager: ResourceManager<'a>
}

impl<'a> FrameGraph<'a> {
    pub fn new(render_context: &'a RenderContext) -> FrameGraph<'a> {
        let resource_manager = ResourceManager::new(
            render_context.get_instance(),
            render_context.get_device_wrapper(),
            render_context.get_physical_device());
        FrameGraph {
            // nodes: Dag::new(),
            // nodes: vec![],
            nodes: stable_graph::StableDiGraph::new(),
            // sorted_nodes: vec![],
            frame_started: false,
            compiled: false,
            pipeline_manager: PipelineManager::new(),
            resource_manager
        }
    }

    pub fn start(&mut self) {
        assert!(!self.frame_started, "Can't start a frame that's already been started");
        self.frame_started = true;
    }

    pub fn add_node(&mut self, node: PassNode) {
        assert!(self.frame_started, "Can't add PassNode before frame has been started");
        assert!(!self.compiled, "Can't add PassNode after frame has been compiled");
        let node_index = self.nodes.add_node(node);
        // self.nodes.push(node);
    }

    pub fn compile(&mut self) {
        assert!(self.frame_started, "Can't compile FrameGraph before it's been started");
        assert!(!self.compiled, "FrameGraph has already been compiled");

        // create input/output maps to detect graph edges
        let mut input_map = MultiMap::new();
        let mut output_map = MultiMap::new();
        for node_index in self.nodes.node_indices() {
            let node = &self.nodes[node_index];
            for input in node.get_inputs() {
                input_map.insert(*input, node_index);
            }
            for rt in node.get_rendertargets() {
                output_map.insert(*rt, node_index);
            }
        }

        // iterate over input map. For each input, find matching outputs and then
        // generate a graph edge for each pairing
        let mut unresolved_passes = Vec::new();
        for (input, node_index) in input_map.iter() {
            let find_outputs = output_map.get_vec(input);
            match find_outputs {
                Some(matched_outputs) => {
                    // input/output match defines a graph edge
                    for matched_output in matched_outputs {
                        self.nodes.add_edge(
                            *node_index,
                            *matched_output,
                            0);
                    }
                },
                _ => {
                    unresolved_passes.push(node_index);
                }
            }
        }

        // need to also do a pass over the output map just to find unused
        // passes which can be culled from the framegraph
        let mut unused_passes = Vec::new();
        for (output, node_index) in output_map.iter() {
            let find_inputs = input_map.get_vec(output);
            match find_inputs {
                Some(_) => {
                    // These would have already been found during the input map pass
                },
                _ => {
                    unused_passes.push(node_index);
                }
            }
        }

        for unresolved_pass in unresolved_passes {
            println!("Pass has an unresolved input; removing from graph: {}",
                self.nodes.node_weight(*unresolved_pass).unwrap()
                    .get_pipeline_description().get_name());
            // self.nodes.remove_node(*unresolved_pass);
        }

        for unused_pass in unused_passes {
            println!("Pass has an unused output; removing from graph: {}",
                     self.nodes.node_weight(*unused_pass).unwrap()
                         .get_pipeline_description().get_name());
            // self.nodes_remove_node(*unused_pass);
        }

        // unresolved and unused passes have been removed from the graph,
        // so now we can use a topological sort to generate an execution order
        // let sort_result = daggy::petgraph::algo::toposort(&self.nodes, None);

        self.compiled = true;
    }

    pub fn end(&mut self, render_context: &mut RenderContext, command_buffer: vk::CommandBuffer) {
        assert!(self.frame_started, "Can't end frame before it's been started");
        assert!(self.compiled, "Can't end frame before it's been compiled");
        // let mut next = self.nodes.pop();
        // while next.is_some() {
        //     let node = next.unwrap();
        //     // TODO: determine the actual renderpass to provide
        //     let pipeline = self.pipeline_manager.create_pipeline(render_context, vk::RenderPass::null(), node.get_pipeline_description() );
        //     // let mut resolved_inputs: Vec<ResolvedResource> = Vec::new();
        //     let mut resolved_inputs = ResolvedResourceMap::new();
        //     let mut resolved_outputs = ResolvedResourceMap::new();
        //     let inputs = node.get_inputs().as_ref();
        //     let outputs = node.get_outputs().as_ref();
        //     for input in inputs {
        //         let resolved = self.resource_manager.resolve_resource(input);
        //         resolved_inputs.insert(input.clone(), resolved.clone());
        //     }
        //     for output in outputs {
        //         let resolved = self.resource_manager.resolve_resource(output);
        //         resolved_outputs.insert(output.clone(), resolved.clone());
        //     }
        //     node.execute(
        //         render_context,
        //         command_buffer,
        //         &resolved_inputs,
        //         &resolved_outputs);
        //     next = self.nodes.pop();
        // }
    }
}