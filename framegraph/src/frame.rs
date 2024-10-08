use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use ash::vk;
use petgraph::stable_graph::{StableDiGraph, NodeIndex};
use api_types::device::DeviceWrapper;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::pass_type::PassType;

#[derive(Eq, PartialEq, Debug)]
enum FrameState {
    New,
    Started,
    Ended
}

pub struct Frame {
    pub nodes: StableDiGraph<PassType, u32>,
    root_index: Option<NodeIndex>,
    state: FrameState,
    pub sorted_nodes: Vec<NodeIndex>,
    device: Rc<RefCell<DeviceWrapper>>,
    pub(crate) descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>
}

impl Debug for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("state", &self.state)
            .finish()
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        log::trace!(target: "frame", "Dropping frame");
        unsafe {
            self.device.borrow().get().free_descriptor_sets(
                self.descriptor_pool,
                &self.descriptor_sets)
                .expect("Failed to free Descriptor Sets for Frame");
        }
    }
}

impl Frame {
    pub fn new(device: Rc<RefCell<DeviceWrapper>>, descriptor_pool: vk::DescriptorPool) -> Self {
        Frame {
            nodes: StableDiGraph::new(),
            root_index: None,
            state: FrameState::New,
            sorted_nodes: Vec::new(),
            device,
            descriptor_pool,
            descriptor_sets: Vec::new()
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