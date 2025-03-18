use std::ops::Deref;
use crate::compute_pass_node::ComputePassNode;
use crate::copy_pass_node::CopyPassNode;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::pass_node::PassNode;
use crate::present_pass_node::PresentPassNode;

#[derive(Debug)]
pub enum PassType<'d> {
    Graphics(GraphicsPassNode<'d>),
    Copy(CopyPassNode<'d>),
    Compute(ComputePassNode<'d>),
    Present(PresentPassNode<'d>)
}

// TODO: this could definitely be handled as a macro
impl<'d> Deref for PassType<'d> {
    type Target = dyn PassNode<'d> + 'd;

    // fn deref(& self) -> &Self::Target {
    fn deref(& self) -> &(dyn PassNode<'d> + 'd) {
        match self {
            PassType::Graphics(gn) => {
                gn
            },
            PassType::Copy(cn) => {
                cn
            },
            PassType::Compute(cn) => {
                cn
            },
            PassType::Present(pn) => {
                pn
            }
        }
    }
}