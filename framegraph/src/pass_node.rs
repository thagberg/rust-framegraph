use std::collections::HashMap;
use std::fmt::Debug;
use ash::vk;
use context::render_context::{RenderContext, CommandBuffer};
use crate::attachment::AttachmentReference;
use crate::barrier::{BufferBarrier, ImageBarrier};
use crate::resource::resource_manager::ResourceManager;
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResourceMap, ResolvedResource};
use crate::binding::{ResourceBinding, ResolvedResourceBinding};

pub type ResolvedBindingMap = HashMap<ResourceHandle, ResolvedResourceBinding>;

pub trait PassNode {
    type RC;
    type CB;
    type PD;

    fn get_name(&self) -> &str;

    fn get_inputs(&self) -> &[ResourceBinding];

    fn get_inputs_mut(&mut self) -> &mut [ResourceBinding];

    fn get_outputs(&self) -> &[ResourceBinding];

    fn get_outputs_mut(&mut self) -> &mut [ResourceBinding];

    fn get_rendertargets(&self) -> &[AttachmentReference];

    fn get_rendertargets_mut(&mut self) -> &mut [AttachmentReference];

    fn get_copy_sources(&self) -> &[ResourceHandle];

    fn get_copy_dests(&self) -> &[ResourceHandle];

    fn get_pipeline_description(&self) -> &Option<Self::PD>;

    fn get_dependencies(&self) -> Vec<ResourceHandle>;

    fn get_writes(&self) -> Vec<ResourceHandle>;

    fn get_buffer_barriers(&self) -> &[BufferBarrier];

    fn get_image_barriers(&self) -> &[ImageBarrier];

    fn add_image_barrier(&mut self, image_barrier: ImageBarrier);

    fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB,
        resolved_inputs: &ResolvedBindingMap,
        resolved_outputs: &ResolvedBindingMap,
        resolved_copy_sources: &ResolvedResourceMap,
        resolved_copy_dests: &ResolvedResourceMap);
}