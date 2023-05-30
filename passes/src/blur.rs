use std::cell::RefCell;
use std::rc::Rc;

use ash::vk;

use context::api_types::device::DeviceResource;
use framegraph::graphics_pass_node::GraphicsPassNode;
use context::vulkan_render_context::VulkanRenderContext;

pub fn generate_pass(
    source: Rc<RefCell<DeviceResource>>
) -> GraphicsPassNode {


    GraphicsPassNode::builder("blur".to_string())
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                  command_buffer: &vk::CommandBuffer | {

            }
        ))
        .build()
        .expect("Failed to create blur passnode")
}