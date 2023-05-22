use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use glam::IVec2;
use context::api_types::device::{DeviceResource, ResourceType};

use context::api_types::vulkan_command_buffer::VulkanCommandBuffer;
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::graphics_pass_node::GraphicsPassNode;

pub fn generate_pass(
    source: Rc<RefCell<DeviceResource>>,
    source_layer: u32,
    dest: Rc<RefCell<DeviceResource>>,
    dest_layer: u32,
    offsets: [IVec2; 2]) -> GraphicsPassNode {

    GraphicsPassNode::builder("blit".to_string())
        .copy_src(source.clone())
        .copy_dst(dest.clone())
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                    command_buffer: &vk::CommandBuffer| {

                unsafe {
                    let resolved_source = source.borrow();
                    let resolved_dest = dest.borrow();

                    let source_image = {
                        if let Some(s) = &resolved_source.resource_type {
                            if let ResourceType::Image(s) = s {
                                s
                            } else {
                                panic!("Image expected as source for blit");
                            }
                        } else {
                            panic!("Source is an unresolved resource for blit");
                        }
                    };

                    let dest_image = {
                        if let Some(d) = &resolved_dest.resource_type {
                            if let ResourceType::Image(d) = d {
                                d
                            } else {
                                panic!("Image expected as dest for blit");
                            }
                        } else {
                            panic!("Dest is an unresolved resource for blit");
                        }
                    };

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
                    render_ctx.get_device().borrow().get().cmd_blit_image(
                        *command_buffer,
                        source_image.image,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        dest_image.image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        std::slice::from_ref(&blit_region),
                        vk::Filter::LINEAR);
                }
        }))
        .build()
        .expect("Failed to create Blit passnode")
}