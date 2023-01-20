use crate::frame::Frame;

pub trait FrameGraph
{
    type PN;
    type RPM;
    type RM;
    type PM;
    type CB;
    type RC;
    type Index;

    fn start(&mut self) -> Frame;

    fn end(&mut self, frame: Frame, command_buffer: &Self::CB);

    // fn end(
    //     &mut self,
    //     resource_manager: &mut Self::RM,
    //     render_context: &mut Self::RC,
    //     command_buffer: &Self::CB);
}
