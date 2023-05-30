use petgraph::stable_graph::{StableDiGraph, NodeIndex};
use crate::graphics_pass_node::GraphicsPassNode;
use crate::pass_type::PassType;

#[derive(Eq, PartialEq)]
enum FrameState {
    New,
    Started,
    Ended
}

pub struct Frame {
    pub nodes: StableDiGraph<PassType, u32>,
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

    pub fn add_node(&mut self, node: PassType) -> NodeIndex {
        assert!(self.state == FrameState::Started, "Frame must be started before adding nodes");
        self.nodes.add_node(node)
    }

    pub fn start(&mut self, root_node: PassType) {
        assert!(self.state == FrameState::New, "Frame has already been started");
        self.state = FrameState::Started;
        self.root_index = Some(self.add_node(root_node));
    }

    pub (crate) fn end(&mut self) {
        assert!(self.state == FrameState::Started, "Frame must be in Started state to be ended");
        self.state = FrameState::Ended;
    }

    pub (crate) fn get_root_index(&self) -> NodeIndex {
        assert!(self.state != FrameState::New, "Cannot get root index before the Frame has been started");
        self.root_index.expect("Something bad happened; a Frame was started without a root node")
    }
}