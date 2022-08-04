use std::fmt::Debug;
use context::i_render_context::{RenderContext, CommandBuffer};
use crate::resource::resource_manager::{ResourceHandle, ResolvedResourceMap};

pub trait PassNode<RCType: RenderContext, CBType: CommandBuffer> {
    fn get_name(&self) -> &str;

    fn get_inputs(&self) -> &[ResourceHandle];

    fn get_outputs(&self) -> &[ResourceHandle];

    fn get_rendertargets(&self) -> &[ResourceHandle];

    fn execute(
        &self,
        render_context: &mut RCType,
        command_buffer: &CBType,
        resolved_inputs: &ResolvedResourceMap,
        resolved_outputs: &ResolvedResourceMap);
}