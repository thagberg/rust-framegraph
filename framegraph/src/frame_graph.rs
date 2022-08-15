extern crate petgraph;
use petgraph::{graph, stable_graph, Direction, Directed};
use petgraph::stable_graph::{Edges, NodeIndex};
extern crate multimap;
use multimap::MultiMap;

extern crate context;
use context::render_context::{RenderContext, CommandBuffer};

use ash::vk;
use crate::pass_node::PassNode;
// use crate::pass_node::{GraphicsPassNode};
use crate::pipeline::{PipelineManager, VulkanPipelineManager};
use crate::resource::resource_manager::ResourceManager;
use crate::resource::vulkan_resource_manager::{ResolvedResourceMap, ResourceHandle};
use crate::renderpass_manager::{RenderpassManager, VulkanRenderpassManager};

use std::collections::HashMap;
use std::marker::PhantomData;
use petgraph::visit::EdgeRef;


pub struct FrameGraph<RPM, PM>
    where
        RPM: RenderpassManager,
        RPM::PN: PassNode,
        PM: PipelineManager<RC = <<RPM as RenderpassManager>::PN as PassNode>::RC> {

    nodes: stable_graph::StableDiGraph<RPM::PN, u32>,
    frame_started: bool,
    compiled: bool,
    pipeline_manager: PM,
    renderpass_manager: RPM,
    sorted_nodes: Option<Vec<NodeIndex>>,
    root_index: Option<NodeIndex>
}

impl<RPM, PM> FrameGraph<RPM, PM>
    where
        RPM: RenderpassManager,
        RPM::PN: PassNode,
        PM: PipelineManager<RC = <<RPM as RenderpassManager>::PN as PassNode>::RC> {

    pub fn new(renderpass_manager: RPM, pipeline_manager: PM) -> FrameGraph<RPM, PM> {
        FrameGraph {
            nodes: stable_graph::StableDiGraph::new(),
            frame_started: false,
            compiled: false,
            pipeline_manager,
            renderpass_manager,
            sorted_nodes: None,
            root_index: None
        }
    }

    pub fn start(&mut self, root_node: RPM::PN) {
        assert!(!self.frame_started, "Can't start a frame that's already been started");
        self.frame_started = true;
        self.root_index = Some(self.add_node(root_node));
    }

    pub fn add_node(&mut self, node: RPM::PN) -> NodeIndex {
        assert!(self.frame_started, "Can't add PassNode before frame has been started");
        assert!(!self.compiled, "Can't add PassNode after frame has been compiled");
        self.nodes.add_node(node)
    }

    fn _mark_unused(&self, visited_nodes: &mut Vec<bool>, edges: Edges<u32, Directed>)  {
        for edge in edges {
            let node_index = edge.source();
            visited_nodes[node_index.index()] = true;
            let incoming = self.nodes.edges_directed(node_index, Direction::Incoming);
            self._mark_unused(visited_nodes, incoming);
        }
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
                            *matched_output,
                            *node_index,
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

        // Ensure root node is valid, then mark any passes which don't contribute to root node as unused
        let mut visited_nodes: Vec<bool> = vec![false; self.nodes.node_count()];
        match self.root_index {
            Some(root_index) => {
                let root_node = self.nodes.node_weight(root_index);
                match root_node {
                    Some(node) => {
                        visited_nodes[root_index.index()] = true;
                        let mut incoming = self.nodes.edges_directed(root_index, Direction::Incoming);
                        self._mark_unused(&mut visited_nodes, incoming);
                    },
                    None => {
                        panic!("Root node is invalid");
                    }
                }
            },
            None => {
                panic!("Root node was elided from frame graph, might have an unresolved dependency");
            }
        }

        // now remove unused passes
        for i in 0..visited_nodes.len() {
            if visited_nodes[i] == false {
                let node_index = NodeIndex::new(i);
                {
                    let node = self.nodes.node_weight(node_index).unwrap();
                    println!("Removing unused node: {:?}", node.get_name());
                }
                self.nodes.remove_node(node_index);
            }
        }

        if self.root_index.is_some() {
            let root_node = self.nodes.node_weight(self.root_index.unwrap());
        } else {
            panic!("Root node was elided from frame graph, might have an unresolved dependency");
        }

        // unresolved and unused passes have been removed from the graph,
        // so now we can use a topological sort to generate an execution order
        let sort_result = petgraph::algo::toposort(&self.nodes, None);
        match sort_result {
            Ok(sorted_list) => {
                for i in &sorted_list {
                    println!("Node: {:?}", self.nodes.node_weight(*i).unwrap().get_name());
                }
                self.sorted_nodes = Some(sorted_list);
            },
            Err(cycle_error) => {
                println!("A cycle was detected in the framegraph: {:?}", cycle_error);
            }
        }

        // iterate over sorted nodes to generate / fetch renderpasses

        self.compiled = true;
    }

    pub fn end(
        &mut self,
        resource_manager: &mut <RPM as RenderpassManager>::RM,
        render_context: &mut <<RPM as RenderpassManager>::PN as PassNode>::RC,
        command_buffer: &<<RPM as RenderpassManager>::PN as PassNode>::CB)
        where <RPM as RenderpassManager>::RM: ResourceManager, <<RPM as RenderpassManager>::PN as PassNode>::RC: RenderContext {

        assert!(self.frame_started, "Can't end frame before it's been started");
        assert!(self.compiled, "Can't end frame before it's been compiled");
        match &self.sorted_nodes {
            Some(indices) => {
                for index in indices {
                    let node = self.nodes.node_weight(*index).unwrap();
                    let renderpass = self.renderpass_manager.create_or_fetch_renderpass(
                        node,
                        resource_manager,
                        render_context);
                    let pipeline = self.pipeline_manager.create_pipeline(render_context, renderpass, node.get_pipeline_description());
                    //let pipeline = self.pipeline_manager.create_pipeline(render_context, renderpass, node.)
                    let mut resolved_inputs = ResolvedResourceMap::new();
                    let mut resolved_outputs = ResolvedResourceMap::new();
                    let inputs = node.get_inputs().as_ref();
                    let outputs = node.get_outputs().as_ref();
                    let render_targets = node.get_rendertargets().as_ref();
                    for input in inputs {
                        let resolved = resource_manager.resolve_resource(input);
                        resolved_inputs.insert(input.clone(), resolved.clone());
                    }
                    for output in outputs {
                        let resolved = resource_manager.resolve_resource(output);
                        resolved_outputs.insert(output.clone(), resolved.clone());
                    }

                    node.execute(
                        render_context,
                        command_buffer,
                        &resolved_inputs,
                        &resolved_outputs);
                }
            },
            _ => {
                println!("No nodes in framegraph to traverse");
            }
        }
    }
}