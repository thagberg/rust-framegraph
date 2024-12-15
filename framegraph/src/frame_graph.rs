use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use api_types::device::interface::DeviceInterface;
use crate::frame::Frame;

pub trait FrameGraph<'a>
{
    type PN;
    type RPM;
    type PM;
    type CB;
    type RC;
    type Index;

    fn start(
        &mut self,
        device: &'a DeviceInterface,
        descriptor_pool: vk::DescriptorPool) -> Box<Frame<'a>>;

    fn end(
        &mut self,
        frame: &mut Frame,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB);
}
