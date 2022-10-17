pub trait FrameGraph
{
    type PN;
    type RPM;
    type RM;
    type PM;
    type CB;
    type RC;
    type Index;

    fn start(&mut self, root_node: Self::PN);

    fn compile(&mut self);

    fn end(
        &mut self,
        resource_manager: &mut Self::RM,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB);

    fn add_node(&mut self, node: Self::PN) -> Self::Index;
}
