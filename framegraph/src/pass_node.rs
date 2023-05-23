use std::cell::RefCell;
use std::rc::Rc;
use context::api_types::device::DeviceResource;
use crate::attachment::AttachmentReference;
use crate::binding::{ResourceBinding};

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