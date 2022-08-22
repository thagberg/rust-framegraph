use std::fmt::Debug;
use context::render_context::{RenderContext, CommandBuffer};
use crate::resource::resource_manager::ResourceManager;
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResourceMap};

pub trait PassNode {
    type RC;
    type CB;
    type PD;

    fn get_name(&self) -> &str;

    fn get_inputs(&self) -> &[ResourceHandle];

    fn get_outputs(&self) -> &[ResourceHandle];

    fn get_rendertargets(&self) -> &[ResourceHandle];

    fn get_pipeline_description(&self) -> &Option<Self::PD>;

    fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB,
        resolved_inputs: &ResolvedResourceMap,
        resolved_outputs: &ResolvedResourceMap,
        resolved_render_targets: &ResolvedResourceMap);
}