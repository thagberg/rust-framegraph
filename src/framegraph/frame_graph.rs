use ash::vk;
use crate::{PassNode, PassNodeBuilder};

pub struct FrameGraph<'a> {
    nodes: Vec<&'a PassNode>,
    frame_started: bool
}

impl<'a> FrameGraph<'a> {
    pub fn new() -> FrameGraph<'a> {
        FrameGraph {
            nodes: vec![],
            frame_started: false
        }
    }

    pub fn start(&mut self) {
        assert!(!self.frame_started, "Can't start a frame that's already been started");
        self.frame_started = true;
    }

    pub fn add_node(&mut self, node: &'a PassNode) {
        assert!(self.frame_started, "Can't add PassNode before frame has been started");
        self.nodes.push(node);
    }

    pub fn compile(&mut self) {
        assert!(self.frame_started, "Can't compile FrameGraph before it's been started");
    }

    pub fn end(&mut self) {
        assert!(self.frame_started, "Can't end frame before it's been started");
        let mut next = self.nodes.pop();
        while next.is_some() {
            next.unwrap().execute();
            next = self.nodes.pop();
        }
    }
}