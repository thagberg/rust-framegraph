use std::collections::HashMap;
use petgraph::stable_graph::{StableDiGraph, Edges, NodeIndex};
use context::api_types::buffer::BufferCreateInfo;
use context::api_types::image::ImageCreateInfo;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::resource::vulkan_resource_manager::{ResourceCreateInfo, ResourceHandle, VulkanResourceManager};

enum FrameState {
    New,
    Started,
    Ended
}

pub struct Frame<'a> {
    resource_manager: &'a VulkanResourceManager,
    nodes: StableDiGraph<GraphicsPassNode, u32>,
    root_index: Option<NodeIndex>,
    create_info: HashMap<ResourceHandle, ResourceCreateInfo>,
    state: FrameState
}

impl Frame {
    pub fn new(resource_manager: &VulkanResourceManager) -> Self {
        Frame {
            resource_manager,
            nodes: StableDiGraph::new(),
            root_index: None,
            create_info: HashMap::new(),
            state: FrameState::New
        }
    }

    pub fn add_node(&mut self, node: GraphicsPassNode) -> NodeIndex {
        assert(self.state == FrameState::Started, "Frame must be started before adding nodes");
        self.nodes.add_node(node)
    }

    pub fn start(&mut self, root_node: GraphicsPassNode) {
        assert(self.state == FrameState::New, "Frame has already been started");
        self.root_index = Some(self.add_node(root_node));
    }

    pub fn create_image(&mut self, create_info: ImageCreateInfo) -> ResourceHandle {
        assert(self.state == FrameState::Started, "Frame must be in Started state to add image");
        // reserve handle from resource manager
        let new_handle = self.resource_manager.reserve_handle();

        // store mapping of handle -> createinfo locally
        self.create_info.insert(new_handle, ResourceCreateInfo::Image(create_info));

        // return handle
        new_handle
    }

    pub fn create_buffer(&mut self, create_info: BufferCreateInfo) -> ResourceHandle {
        assert(self.state == FrameState::Started, "Frame must be in Started state to add buffer");
        let new_handle = self.resource_manager.reserve_handle();
        self.create_info.insert(new_handle, ResourceCreateInfo::Buffer(create_info));
        new_handle
    }

    pub (in crate::vulkan_frame_graph) fn end(&mut self) {
        assert(self.state == FrameState::Started, "Frame must be in Started state to be ended");
        self.state = FrameState::Ended;
    }

    pub (in crate::vulkan_frame_graph) fn take_nodes(&mut self) -> StableDiGraph<GraphicsPassNode, u32>{
        assert(self.state == FrameState::Ended, "Frame must be ended before taking nodes");
        std::mem::replace(&mut self.nodes, StableDiGraph::new())
    }

    pub (in crate::vulkan_frame_graph) fn take_create_info(&mut self) -> HashMap<ResourceHandle, ResourceCreateInfo> {
        assert(self.state == FrameState::Ended, "Frame must be ended before taking create info");
        std::mem::replace(&mut self.create_info, HashMap::new())
    }

    pub (in crate::vulkan_frame_graph) fn get_root_index(&self) -> NodeIndex {
        assert(self.state != FrameState::New, "Cannot get root index before the Frame has been started");
        self.root_index.expect("Something bad happened; a Frame was started without a root node")
    }
}