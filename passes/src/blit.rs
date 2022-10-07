use ash::vk;
use glam::IVec2;

use context::api_types::vulkan_command_buffer::VulkanCommandBuffer;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResourceMap, ResourceType};
use framegraph::graphics_pass_node::GraphicsPassNode;

pub fn generate_pass(
    source: ResourceHandle,
    source_layer: u32,
    dest: ResourceHandle,
    dest_layer: u32,
    offsets: [IVec2; 2]) -> GraphicsPassNode {

    GraphicsPassNode::builder("blit".to_string())
        .read(source)
        .render_target(dest)
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                    command_buffer: &vk::CommandBuffer,
                    inputs: &ResolvedResourceMap,
                    outputs: &ResolvedResourceMap,
                    render_targets: &ResolvedResourceMap,
                    resolved_copy_sources: &ResolvedResourceMap,
                    resolved_copy_dests: &ResolvedResourceMap| {

                println!("Performing blit");
                let dest_resolved = render_targets.get(&dest).expect("No blit destination");
                let source_resolved = inputs.get(&source).expect("No blit source");
                match (&dest_resolved.resource, &source_resolved.resource) {
                    (ResourceType::Image(d), ResourceType::Image(s)) => {
                        unsafe {
                            let offsets = [
                                vk::Offset3D::builder().x(offsets[0].x).y(offsets[0].y).z(0).build(),
                                vk::Offset3D::builder().x(offsets[1].x).y(offsets[1].y).z(1).build()
                            ];
                            let source_layer = vk::ImageSubresourceLayers::builder()
                                .layer_count(1)
                                .base_array_layer(source_layer)
                                .mip_level(0)
                                .aspect_mask(vk::ImageAspectFlags::COLOR);
                            let dest_layer = vk::ImageSubresourceLayers::builder()
                                .layer_count(1)
                                .base_array_layer(dest_layer)
                                .mip_level(0)
                                .aspect_mask(vk::ImageAspectFlags::COLOR);
                            let blit_region = vk::ImageBlit::builder()
                                .src_subresource(*source_layer)
                                .dst_subresource(*dest_layer)
                                .src_offsets(offsets)
                                .dst_offsets(offsets);
                            render_ctx.get_device().cmd_blit_image(
                                *command_buffer,
                                s.image,
                                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                                d.image,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                std::slice::from_ref(&blit_region),
                                vk::Filter::LINEAR);
                        }
                    },
                    _ => {
                        panic!("Incompatible resource types for blit")
                    }
                }
        }))
        .build()
        .expect("Failed to create Blit passnode")
}