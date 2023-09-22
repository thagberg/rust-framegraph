use ash::vk;
use petgraph::graph::NodeIndex;

pub struct QueueWait {
    pub wait_stage_mask: vk::PipelineStageFlags
}

pub struct CommandList {
    pub nodes: Vec<NodeIndex>,
    pub wait: Option<QueueWait>
}

impl CommandList {
    pub fn new() -> Self {
        CommandList {
            nodes: vec![],
            wait: None,
        }
    }
}