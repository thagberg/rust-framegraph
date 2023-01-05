use ash::vk;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_node::ResolvedBindingMap;
use framegraph::resource::vulkan_resource_manager::ResolvedResourceMap;

pub fn generate_pass() -> GraphicsPassNode {
    GraphicsPassNode::builder("imgui".to_string())
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                    command_buffer: &vk::CommandBuffer,
                    inputs: &ResolvedBindingMap,
                    outputs: &ResolvedBindingMap,
                    resolved_copy_sources: &ResolvedResourceMap,
                    resolved_copy_dests: &ResolvedResourceMap| {

            }
        ))
        .build()
        .expect("Failed to create imgui passnode")
}