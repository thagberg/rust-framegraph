use std::sync::Arc;
use std::sync::Mutex;
use ash::vk;
use ash::vk::{wl_display, ImageAspectFlags};
use api_types::device::interface::DeviceInterface;
use api_types::device::resource::DeviceResource;
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::binding::{BindingInfo, BindingType, ImageBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use profiling::{enter_gpu_span, enter_span};

pub fn clear<'d>(
    target: Arc<Mutex<DeviceResource<'d>>>,
    aspect_mask: vk::ImageAspectFlags) -> PassType<'d>{

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

    let pass_name = {
        if aspect_mask == vk::ImageAspectFlags::COLOR {
            "Color clear".to_string()
        } else if aspect_mask & vk::ImageAspectFlags::DEPTH == vk::ImageAspectFlags::DEPTH {
            "Depth clear".to_string()
        } else {
            panic!("Invalid aspect mask for clear");
        }
    };

    let target_clone = target.clone();
    let pass_node = GraphicsPassNode::builder(pass_name.clone())
        .write(target_binding)
        .fill_commands(Box::new(
            move |device: &DeviceInterface,
                  command_buffer: vk::CommandBuffer | {

                enter_span!(tracing::Level::TRACE, "clear");
                //let borrowed_device = device.borrow();
                enter_gpu_span!(&pass_name, "misc", device.get(), &command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);

                let range = vk::ImageSubresourceRange::default()
                    .aspect_mask(aspect_mask)
                    .level_count(1)
                    .base_mip_level(0)
                    .layer_count(1)
                    .base_array_layer(0);

                unsafe {
                    let image = target_clone.lock().unwrap().get_image().image;
                    if aspect_mask == vk::ImageAspectFlags::COLOR {
                        device.get().cmd_clear_color_image(
                            command_buffer,
                            image,
                            vk::ImageLayout::GENERAL,
                            &Default::default(),
                            std::slice::from_ref(&range));
                    } else if aspect_mask & vk::ImageAspectFlags::DEPTH == vk::ImageAspectFlags::DEPTH {
                        device.get().cmd_clear_depth_stencil_image(
                            command_buffer,
                            image,
                            vk::ImageLayout::GENERAL,
                            &vk::ClearDepthStencilValue::default()
                                .depth(1.0)
                                .stencil(0),
                            std::slice::from_ref(&range));
                    } else {
                        panic!("Invalid aspect mask for clear: {:?}", aspect_mask);
                    }
                };
            }
        ))
        .build()
        .expect("Failed to create color clear pass node");

    PassType::Graphics(pass_node)
}