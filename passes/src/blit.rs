use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use ash::vk;
use glam::IVec2;
use api_types::device::resource::DeviceResource;
use api_types::device::interface::DeviceInterface;

use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::copy_pass_node::CopyPassNode;
use framegraph::pass_type::PassType;
use profiling::{enter_gpu_span, enter_span};

pub fn generate_pass<'d>(
    source: Arc<Mutex<DeviceResource<'d>>>,
    source_layer: u32,
    dest: Arc<Mutex<DeviceResource<'d>>>,
    dest_layer: u32,
    offsets: [IVec2; 2]) -> PassType<'d> {

    let pass_node = CopyPassNode::builder("blit".to_string())
        .copy_src(source.clone())
        .copy_dst(dest.clone())
        .fill_commands(Box::new(
            move |device: &DeviceInterface,
                    command_buffer: vk::CommandBuffer| {

                enter_span!(tracing::Level::TRACE, "Blit");
                enter_gpu_span!("Blit GPU", "Passes", device.get(), &command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);

                unsafe {
                    let resolved_source = source.lock().unwrap();
                    let resolved_dest = dest.lock().unwrap();

                    let source_image = resolved_source.get_image();
                    let dest_image = resolved_dest.get_image();

                    let offsets = [
                        vk::Offset3D::default().x(offsets[0].x).y(offsets[0].y).z(0),
                        vk::Offset3D::default().x(offsets[1].x).y(offsets[1].y).z(1)
                    ];
                    let source_layer = vk::ImageSubresourceLayers::default()
                        .layer_count(1)
                        .base_array_layer(source_layer)
                        .mip_level(0)
                        .aspect_mask(vk::ImageAspectFlags::COLOR);
                    let dest_layer = vk::ImageSubresourceLayers::default()
                        .layer_count(1)
                        .base_array_layer(dest_layer)
                        .mip_level(0)
                        .aspect_mask(vk::ImageAspectFlags::COLOR);
                    let blit_region = vk::ImageBlit::default()
                        .src_subresource(source_layer)
                        .dst_subresource(dest_layer)
                        .src_offsets(offsets)
                        .dst_offsets(offsets);
                    device.get().cmd_blit_image(
                        command_buffer,
                        source_image.image,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        dest_image.image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        std::slice::from_ref(&blit_region),
                        vk::Filter::LINEAR);
                }
        }))
        .build()
        .expect("Failed to create Blit passnode");

        PassType::Copy(pass_node)
}