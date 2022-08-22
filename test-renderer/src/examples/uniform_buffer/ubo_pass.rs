use core::ffi::c_void;

use ash::vk;

use context::api_types::vulkan_command_buffer::VulkanCommandBuffer;
use context::vulkan_render_context::VulkanRenderContext;

use framegraph::graphics_pass_node::{GraphicsPassNode};
use framegraph::resource::vulkan_resource_manager::{ResourceHandle, ResolvedResourceMap, VulkanResourceManager};
use framegraph::pipeline::{PipelineDescription, RasterizationType, DepthStencilType, BlendType, Pipeline};

pub struct OffsetUBO {
    pub offset: [f32; 3]
}

pub struct UBOPass {
    uniform_buffer: ResourceHandle
}

impl Drop for UBOPass {
    fn drop(&mut self) {

    }
}

impl UBOPass {
    pub fn new(resource_manager: &mut VulkanResourceManager) -> Self {
        let ubo_create_info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            size: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let uniform_buffer = resource_manager.create_buffer_persistent(&ubo_create_info);
        let ubo_value = OffsetUBO {
            offset: [0.2, 0.1, 0.0]
        };

        resource_manager.update_buffer(&uniform_buffer, |mapped_memory: *mut c_void| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &ubo_value,
                    mapped_memory as *mut OffsetUBO,
                    std::mem::size_of::<OffsetUBO>());
            };
        });

        UBOPass {
            uniform_buffer
        }
    }

    pub fn generate_pass(&self, resource_manager: &mut VulkanResourceManager, rendertarget_extent: vk::Extent2D) -> (GraphicsPassNode, ResourceHandle) {

        let vertex_input_state_create_info = vk::PipelineVertexInputStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineVertexInputStateCreateFlags::empty(),
            vertex_attribute_description_count: 0,
            p_vertex_attribute_descriptions: std::ptr::null(),
            vertex_binding_description_count: 0,
            p_vertex_binding_descriptions: std::ptr::null(),
        };

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_DYNAMIC_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineDynamicStateCreateFlags::empty(),
            dynamic_state_count: dynamic_states.len() as u32,
            p_dynamic_states: dynamic_states.as_ptr()
        };


        let pipeline_description = PipelineDescription::new(
            vertex_input_state_create_info,
            dynamic_state,
            RasterizationType::Standard,
            DepthStencilType::Disable,
            BlendType::None,
            concat!(env!("OUT_DIR"), "/shaders/hello-vert.spv"),
            concat!(env!("OUT_DIR"), "/shaders/hello-frag.spv")
        );

        let render_target = resource_manager.create_image_transient(
            vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_SRGB)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                // .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC)
                .extent(vk::Extent3D::builder()
                    .height(rendertarget_extent.height)
                    .width(rendertarget_extent.width)
                    .depth(1)
                    .build())
                .mip_levels(1)
                .array_layers(1)
                .build());

        let passnode = GraphicsPassNode::builder("ubo_pass".to_string())
            .pipeline_description(pipeline_description)
            .read(self.uniform_buffer)
            .render_target(render_target)
            .fill_commands(Box::new(
                move |render_ctx: &VulkanRenderContext,
                      command_buffer: &vk::CommandBuffer,
                      inputs: &ResolvedResourceMap,
                      outputs: &ResolvedResourceMap,
                      render_targets: &ResolvedResourceMap|
                    {
                        println!("I'm doing something!");
                        // let render_target = outputs.get(&render_target) .expect("No resolved render target");
                        // let ubo = inputs.get(&uniform_buffer)
                        //     .expect("No resolved UBO");
                        // match (&render_target.resource, &ubo.resource) {
                        //     (ResourceType::Image(rt), ResourceType::Buffer(buffer)) => {
                        //         let framebuffer = render_ctx.create_framebuffers(
                        //             render_pass,
                        //             &vk::Extent2D::builder().width(100).height(100),
                        //             std::slice::from_ref(&rt)
                        //         );
                        //
                        //         let descriptor_buffer = vk::DescriptorBufferInfo {
                        //             buffer: *buffer,
                        //             offset: 0,
                        //             range: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize
                        //         };
                        //
                        //         let descriptor_write = vk::WriteDescriptorSet {
                        //             s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                        //             p_next: std::ptr::null(),
                        //             dst_set: vert_shader_module.descriptor_sets[0],
                        //             dst_binding: 0,
                        //             dst_array_element: 0,
                        //             descriptor_count: 1,
                        //             descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                        //             p_buffer_info: &descriptor_buffer,
                        //             ..Default::default()
                        //         };
                        //         let descriptor_write_sets = [descriptor_write];
                        //
                        //         // TODO: move this into framegraph? Not sure how to bind framebuffer first?
                        //         // begin renderpass
                        //         unsafe {
                        //             let clear_value = vk::ClearValue {
                        //                 color: vk::ClearColorValue {
                        //                     float32: [0.1, 0.1, 0.1, 1.0]
                        //                 }
                        //             };
                        //
                        //             let viewport = vk::Viewport::builder()
                        //                 .x(0.0)
                        //                 .y(0.0)
                        //                 .width(100.0)
                        //                 .height(100.0)
                        //                 .min_depth(0.0)
                        //                 .max_depth(1.0)
                        //                 .build();
                        //
                        //             let scissor = vk::Rect2D::builder()
                        //                 .offset(vk::Offset2D{x: 0, y: 0})
                        //                 .extent(vk::Extent2D::builder().width(100).height(100).build())
                        //                 .build();
                        //
                        //             let render_pass_begin = vk::RenderPassBeginInfo::builder()
                        //                 .render_pass(render_pass)
                        //                 .framebuffer(framebuffer[0])
                        //                 .render_area(vk::Rect2D::builder()
                        //                                  .offset(vk::Offset2D{x: 0, y: 0})
                        //                                  .extent(vk::Extent2D::builder().width(100).height(100).build())
                        //                                  .build())
                        //                 .clear_values(std::slice::from_ref(&clear_value));
                        //
                        //             render_ctx.get_device().cmd_set_viewport(
                        //                 command_buffer,
                        //                 0,
                        //                 std::slice::from_ref(&viewport));
                        //
                        //             render_ctx.get_device().cmd_set_scissor(
                        //                 command_buffer,
                        //                 0,
                        //                 std::slice::from_ref(&scissor));
                        //
                        //             render_ctx.get_device().cmd_begin_render_pass(
                        //                 command_buffer,
                        //                 &render_pass_begin,
                        //                 vk::SubpassContents::INLINE);
                        //
                        //             render_ctx.get_device().cmd_bind_pipeline(
                        //                 command_buffer,
                        //                 vk::PipelineBindPoint::GRAPHICS,
                        //                 graphics_pipelines[0])
                        //         }
                        //
                        //         // Draw calls
                        //         unsafe {
                        //             let device = render_ctx.get_device();
                        //             device.update_descriptor_sets(&descriptor_write_sets, &[]);
                        //             device.cmd_bind_descriptor_sets(
                        //                 command_buffer,
                        //                 vk::PipelineBindPoint::GRAPHICS,
                        //                 vert_shader_module.pipeline_layout,
                        //                 0,
                        //                 &vert_shader_module.descriptor_sets,
                        //                 &[]);
                        //             device.cmd_draw(command_buffer, 3, 1, 0, 0);
                        //         }
                        //
                        //         // TODO: move this to framegraph?
                        //         // End renderpass
                        //         unsafe {
                        //             let graphics_queue_index = render_ctx.get_graphics_queue_index();
                        //             render_ctx.get_device().cmd_end_render_pass(command_buffer);
                        //             // let image_transition = vk::ImageMemoryBarrier::builder()
                        //             //     .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        //             //     .dst_access_mask(vk::AccessFlags::SHADER_READ)
                        //             //     .image(rt.image)
                        //             //     .old_layout(vk::ImageLayout::UNDEFINED)
                        //             //     .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        //             //     .src_queue_family_index(graphics_queue_index)
                        //             //     .dst_queue_family_index(graphics_queue_index)
                        //             //     .subresource_range(vk::ImageSubresourceRange::builder()
                        //             //         .aspect_mask(vk::ImageAspectFlags::COLOR)
                        //             //         .base_mip_level(0)
                        //             //         .level_count(1)
                        //             //         .base_array_layer(0)
                        //             //         .layer_count(1)
                        //             //         .build());
                        //             // render_ctx.get_device().cmd_pipeline_barrier(
                        //             //     command_buffer,
                        //             //     vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        //             //     vk::PipelineStageFlags::FRAGMENT_SHADER,
                        //             //     vk::DependencyFlags::empty(),
                        //             //     &[],
                        //             //     &[],
                        //             //     std::slice::from_ref(&image_transition));
                        //         }
                        //     },
                        //     _ => {}
                        // }
                    }
            ))
            .build()
            .expect("Failed to create PassNode");

        return (passnode, render_target);
    }
}