use std::cell::RefCell;
use std::rc::Rc;
use ash::vk;
use context::api_types::device::DeviceResource;
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::binding::{BindingInfo, BindingType, ImageBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;

pub fn clear_color(
    target: Rc<RefCell<DeviceResource>>) -> PassType{

    let target_binding = ResourceBinding {
        resource: target.clone(),
        binding_info: BindingInfo {
            binding_type: BindingType::Image(ImageBindingInfo { layout: vk::ImageLayout::GENERAL}),
            set: 0,
            slot: 0,
            stage: vk::PipelineStageFlags::TRANSFER,
            access: vk::AccessFlags::TRANSFER_WRITE
        }
    };

    let pass_node = GraphicsPassNode::builder("Color Clear".to_string())
        .write(target_binding)
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                  command_buffer: &vk::CommandBuffer | {
                let range = vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .base_mip_level(0)
                    .layer_count(1)
                    .base_array_layer(0)
                    .build();

                unsafe {
                    render_ctx.get_device().borrow().get().cmd_clear_color_image(
                        *command_buffer,
                        target.borrow().get_image().image,
                        vk::ImageLayout::GENERAL,
                        &Default::default(),
                        std::slice::from_ref(&range));
                };
            }
        ))
        .build()
        .expect("Failed to create color clear pass node");

    PassType::Graphics(pass_node)
}