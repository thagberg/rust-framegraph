use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::DeviceResource;
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::binding::ResourceBinding;
use crate::compute_pass_node::ComputePassNode;
use crate::copy_pass_node::CopyPassNode;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::pass_node::PassNode;
use crate::pipeline::PipelineDescription;

pub enum PassType {
    Graphics(GraphicsPassNode),
    Copy(CopyPassNode),
    Compute(ComputePassNode)
}

// TODO: this could definitely be handled as a macro
impl Deref for PassType {
    type Target = dyn PassNode;

    fn deref(&self) -> &Self::Target {
        match self {
            PassType::Graphics(gn) => {
                gn as &dyn PassNode
            },
            PassType::Copy(cn) => {
                cn as &dyn PassNode
            },
            PassType::Compute(cn) => {
                cn as &dyn PassNode
            }
        }
    }
}