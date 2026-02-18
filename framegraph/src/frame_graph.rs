use ash::vk;
use api_types::device::interface::DeviceInterface;
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
        device: DeviceInterface,
        descriptor_pool: vk::DescriptorPool) -> Box<Frame>;

    fn end(
        &mut self,
        frame: &mut Frame,
        // render_context: &'a mut Self::RC,
        render_context: &Self::RC,
        command_buffer: &Self::CB);
}
