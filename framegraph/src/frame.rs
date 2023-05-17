use std::collections::HashMap;
use ash::vk;
use petgraph::stable_graph::{StableDiGraph, Edges, NodeIndex};
use context::api_types::buffer::BufferCreateInfo;
use context::api_types::device::DeviceResource;
use context::api_types::image::ImageCreateInfo;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::pass_node::PassNode;
use crate::resource::vulkan_resource_manager::{ResourceCreateInfo, ResourceHandle, VulkanResourceManager};

#[derive(Eq, PartialEq)]
enum FrameState {
    New,
    Started,
    Ended
}

pub struct Frame {
    pub nodes: StableDiGraph<GraphicsPassNode, u32>,
    root_index: Option<NodeIndex>,
    state: FrameState,
    pub sorted_nodes: Vec<NodeIndex>
}

impl Frame {
    pub fn new() -> Self {
        Frame {
            nodes: StableDiGraph::new(),
            root_index: None,
            state: FrameState::New,
            sorted_nodes: Vec::new()
        }
    }

    pub fn add_node(&mut self, node: GraphicsPassNode) -> NodeIndex {
        assert!(self.state == FrameState::Started, "Frame must be started before adding nodes");
        self.nodes.add_node(node)
    }

    pub fn start(&mut self, root_node: GraphicsPassNode) {
        assert!(self.state == FrameState::New, "Frame has already been started");
        self.state = FrameState::Started;
        self.root_index = Some(self.add_node(root_node));
    }

    pub (crate) fn set_sorted_nodes(&mut self, sorted_nodes: Vec<NodeIndex>) {
        assert!(self.state == FrameState::Ended, "Frame must be ended before setting sort order");
        self.sorted_nodes = sorted_nodes;
    }

    pub (crate) fn get_sorted_nodes(&self) -> &[NodeIndex] {
        &self.sorted_nodes
    }

    pub (crate) fn end(&mut self) {
        assert!(self.state == FrameState::Started, "Frame must be in Started state to be ended");
        self.state = FrameState::Ended;
    }

    pub (crate) fn get_nodes(&mut self) -> &mut StableDiGraph<GraphicsPassNode, u32> {
        assert!(self.state == FrameState::Ended, "Frame must be ended before fetching nodes");
        &mut self.nodes
    }

    pub (crate) fn take_nodes(&mut self) -> StableDiGraph<GraphicsPassNode, u32>{
        assert!(self.state == FrameState::Ended, "Frame must be ended before taking nodes");
        std::mem::replace(&mut self.nodes, StableDiGraph::new())
    }

    pub (crate) fn get_root_index(&self) -> NodeIndex {
        assert!(self.state != FrameState::New, "Cannot get root index before the Frame has been started");
        self.root_index.expect("Something bad happened; a Frame was started without a root node")
    }
}