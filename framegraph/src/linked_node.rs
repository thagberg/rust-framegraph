use ash::vk;
use petgraph::graph::NodeIndex;

pub struct QueueWait {
    pub wait_stage_mask: vk::PipelineStageFlags
}

pub struct LinkedNode {
    pub index: NodeIndex,
    pub wait: Option<QueueWait>
}