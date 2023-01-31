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

    fn start<'a>(&'a mut self, resource_manager: &'a VulkanResourceManager) -> Frame;

    fn end(
        &mut self,
        frame: Frame,
        resource_manager: &Self::RM,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB);
}
