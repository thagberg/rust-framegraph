use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::DeviceWrapper;
use crate::frame::Frame;

pub trait FrameGraph
{
    type PN;
    type RPM;
    type PM;
    type CB;
    type RC;
    type Index;

    fn start(
        &mut self,
        device: Rc<RefCell<DeviceWrapper>>,
        descriptor_pool: vk::DescriptorPool) -> Box<Frame>;

    fn end(
        &mut self,
        frame: &mut Frame,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB);
}
