use crate::frame::Frame;

pub trait FrameGraph
{
    type PN;
    type RPM;
    type PM;
    type CB;
    type RC;
    type Index;

    fn start(&mut self) -> Box<Frame>;

    fn end(
        &mut self,
        frame: &mut Frame,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB);
}
