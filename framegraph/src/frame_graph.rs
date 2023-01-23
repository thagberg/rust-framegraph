use crate::frame::Frame;
use crate::resource::vulkan_resource_manager::VulkanResourceManager;

pub trait FrameGraph
{
    type PN;
    type RPM;
    type RM;
    type PM;
    type CB;
    type RC;
    type Index;

    fn start(&mut self, resource_manager: &VulkanResourceManager) -> Frame;

    fn end(&mut self, mut frame: Frame, command_buffer: &Self::CB);

    // fn end(
    //     &mut self,
    //     resource_manager: &mut Self::RM,
    //     render_context: &mut Self::RC,
    //     command_buffer: &Self::CB);
}
