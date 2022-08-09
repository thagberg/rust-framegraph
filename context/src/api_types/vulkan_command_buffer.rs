use ash::vk;
use crate::render_context::CommandBuffer;

pub struct VulkanCommandBuffer {
    command_buffer: vk::CommandBuffer
}

impl CommandBuffer for VulkanCommandBuffer {

}