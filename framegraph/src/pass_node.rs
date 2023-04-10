use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::DeviceResource;
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

    fn get_copy_sources(&self) -> &[Rc<RefCell<DeviceResource>>];

    fn get_copy_dests(&self) -> &[Rc<RefCell<DeviceResource>>];

    fn get_pipeline_description(&self) -> &Option<Self::PD>;

    fn execute(
        &self,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB);
}