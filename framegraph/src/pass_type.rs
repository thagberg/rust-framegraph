use std::ops::Deref;
use crate::compute_pass_node::ComputePassNode;
use crate::copy_pass_node::CopyPassNode;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::pass_node::PassNode;
use crate::present_pass_node::PresentPassNode;

#[derive(Debug)]
pub enum PassType {
    Graphics(GraphicsPassNode),
    Copy(CopyPassNode),
    Compute(ComputePassNode),
    Present(PresentPassNode)
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
            },
            PassType::Present(pn) => {
                pn as &dyn PassNode
            }
        }
    }
}