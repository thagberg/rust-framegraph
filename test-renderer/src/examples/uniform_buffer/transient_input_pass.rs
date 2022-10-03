use ash::vk;

use crate::context::vulkan_render_context::VulkanRenderContext;
use crate::framegraph::graphics_pass_node::PassNode;
use crate::resource::resource_manager::{ResourceHandle, ResolvedResourceMap};

use untitled::{
    utility::share,
};

pub struct TransientInputPass {
    pub pass_node: PassNode
}

impl TransientInputPass {
    fn generate_renderpass(
        render_context: &mut VulkanRenderContext) -> vk::RenderPass
    {
        let rt_attachment = vk::AttachmentDescription::builder()
            .format(render_context.get_swapchain().as_ref().unwrap().get_format())
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let rt_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::builder()
            .color_attachments(std::slice::from_ref(&rt_attachment_ref))
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

        // TODO: Need to refresh on stage access / masks
        let subpass_dependency = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            // .dst_subpass(0)
            .dst_subpass(0)
            .src_access_mask(vk::AccessFlags::MEMORY_READ)
            .dst_access_mask(vk::AccessFlags::MEMORY_WRITE)
            .src_stage_mask(vk::PipelineStageFlags::TOP_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT);

        let render_pass_create_info = vk::RenderPassCreateInfo::builder()
            .attachments(std::slice::from_ref(&rt_attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&subpass_dependency));

        let render_pass = unsafe {
            render_context.get_device().create_render_pass(&render_pass_create_info, None)
                .expect("Failed to create renderpass for Transient Pass")
        };

        render_pass
    }

    pub fn new(
        render_context: &mut VulkanRenderContext,
        rendertarget_handle: ResourceHandle,
        texture_handle: ResourceHandle) -> Self
    {
        let render_pass = Self::generate_renderpass(render_context);
        // let backbuffer = &render_context.get_swapchain().as_ref().unwrap().get_images()[image_index];
        // create descriptor set layouts
        let texture_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
            // .immutable_samplers(&[]);
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(std::slice::from_ref(&texture_binding));
            // .binding_count(texture_bindings.len() as u32)
            // .p_bindings(texture_bindings.as_ptr());
        let descriptor_set_layouts = unsafe {[
            render_context.get_device().create_descriptor_set_layout(
                &descriptor_set_layout_create_info,
                None)
                .expect("Failed to create descriptor set layout")
        ]};

        // create descriptor sets
        let descriptor_sets = render_context.create_descriptor_sets(&descriptor_set_layouts);

        // create shader modules
        let vert_shader_module = share::create_shader_module(
            render_context.get_device(),
            // include_bytes!(concat!(env!("OUT_DIR"), "/shaders/transient_input-vert.spv")).to_vec()
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/transient_input-vert.spv"))
        );
        let frag_shader_module = share::create_shader_module(
            render_context.get_device(),
            // include_bytes!(concat!(env!("OUT_DIR"), "/shaders/transient_input-frag.spv")).to_vec()
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/transient_input-frag.spv"))
        );
        let main_function_name = std::ffi::CString::new("main").unwrap();
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::builder()
                .flags(vk::PipelineShaderStageCreateFlags::empty())
                .module(vert_shader_module)
                .name(&main_function_name)
                .stage(vk::ShaderStageFlags::VERTEX).build(),
            vk::PipelineShaderStageCreateInfo::builder()
                .flags(vk::PipelineShaderStageCreateFlags::empty())
                .module(frag_shader_module)
                .name(&main_function_name)
                .stage(vk::ShaderStageFlags::FRAGMENT).build()
        ];

        // create pipeline layout
        let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .flags(vk::PipelineVertexInputStateCreateFlags::empty())
            .vertex_attribute_descriptions(&[])
            .vertex_binding_descriptions(&[]);
        let vertex_input_assembly_create_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .flags(vk::PipelineInputAssemblyStateCreateFlags::empty())
            .primitive_restart_enable(false)
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let rasterization_state_create_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .flags(vk::PipelineRasterizationStateCreateFlags::empty())
            .line_width(1.0)
            .depth_clamp_enable(false)
            .cull_mode(vk::CullModeFlags::NONE)
            .rasterizer_discard_enable(false)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false)
            .polygon_mode(vk::PolygonMode::FILL);
        let multiasample_state_create_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let stencil_state = vk::StencilOpState::builder()
            .compare_op(vk::CompareOp::ALWAYS)
            .write_mask(0)
            .reference(0);
        let depth_state_create_info = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(false)
            .depth_write_enable(false)
            .stencil_test_enable(false)
            .front(*stencil_state)
            .back(*stencil_state);
        let color_blend_attachment_states = [
            vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(false)
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .build()
        ];
        let color_blend_state_create_info = vk::PipelineColorBlendStateCreateInfo::builder()
            .attachments(&color_blend_attachment_states);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_create_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&dynamic_states);
        let viewport_state_create_info = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            // .viewports(&[])
            .scissor_count(1);
            // .scissors(&[]);
        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&descriptor_set_layouts);
            // .push_constant_ranges(&[]);
        let pipeline_layout = unsafe {
            render_context.get_device().create_pipeline_layout(
                &pipeline_layout_create_info,
                None
            ).expect("Failed to create pipeline layout")
        };

        // create graphics pipeline
        let graphics_pipeline_create_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_create_info)
            .input_assembly_state(&vertex_input_assembly_create_info)
            .viewport_state(&viewport_state_create_info)
            .rasterization_state(&rasterization_state_create_info)
            .multisample_state(&multiasample_state_create_info)
            .depth_stencil_state(&depth_state_create_info)
            .color_blend_state(&color_blend_state_create_info)
            .dynamic_state(&dynamic_state_create_info)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0)
            .base_pipeline_handle(vk::Pipeline::null())
            .base_pipeline_index(-1);
        let graphics_pipelines = unsafe {
            render_context.get_device().create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&graphics_pipeline_create_info),
                None
            ).expect("Failed to create graphics pipeline")
        };

        // cleanup shader modules
        unsafe {
            render_context.get_device().destroy_shader_module(vert_shader_module, None);
            render_context.get_device().destroy_shader_module(frag_shader_module, None);
        }

        // create PassNode
        let pass_node = PassNode::builder()
            // .renderpass(render_pass)
            // .layout(pipeline_layout)
            // .pipeline(graphics_pipelines[0])
            .read(texture_handle)
            .write(rendertarget_handle)
            .fill_commands(
                Box::new(
                    move |render_context: &VulkanRenderContext,
                          command_buffer: vk::CommandBuffer,
                          inputs: &ResolvedResourceMap,
                          outputs: &ResolvedResourceMap|
                    {
                        // println!("Inside transient pass");
                        // let render_target = outputs.get(&rendertarget_handle)
                        //     .expect("No resolved rendertarget");
                        // let texture = inputs.get(&texture_handle)
                        //     .expect("No resolved texture");
                        // match (&render_target.resource, &texture.resource)
                        // {
                        //     (ResourceType::Image(rt), ResourceType::Image(tex)) => {
                        //         let swapchain = render_context.get_swapchain().as_ref().unwrap();
                        //         let framebuffer = render_context.create_framebuffers(
                        //             render_pass,
                        //             &swapchain.get_extent(),
                        //             std::slice::from_ref(&rt));
                        //
                        //         let sampler_create = vk::SamplerCreateInfo::builder()
                        //             .mag_filter(vk::Filter::LINEAR)
                        //             .min_filter(vk::Filter::LINEAR)
                        //             .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        //             .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        //             .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        //             .anisotropy_enable(false)
                        //             .max_anisotropy(0.0)
                        //             .border_color(vk::BorderColor::FLOAT_OPAQUE_BLACK)
                        //             .unnormalized_coordinates(false)
                        //             .compare_enable(false)
                        //             .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                        //             .min_lod(0.0)
                        //             .max_lod(0.0);
                        //
                        //         let sampler = unsafe {
                        //             render_context.get_device().create_sampler(
                        //                 &sampler_create,
                        //                 None)
                        //                 .expect("Failed to create sampler")
                        //         };
                        //
                        //         let descriptor_image = vk::DescriptorImageInfo::builder()
                        //             .image_view(tex.view)
                        //             .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        //             .sampler(sampler);
                        //
                        //         let descriptor_write = vk::WriteDescriptorSet::builder()
                        //             .dst_set(descriptor_sets[0])
                        //             .dst_binding(0)
                        //             .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        //             .image_info(std::slice::from_ref(&descriptor_image));
                        //
                        //         unsafe {
                        //             let clear_value = vk::ClearValue {
                        //                 color: vk::ClearColorValue {
                        //                     float32: [0.3, 0.1, 0.1, 1.0]
                        //                 }
                        //             };
                        //
                        //             let viewport = vk::Viewport::builder()
                        //                 .x(0.0)
                        //                 .y(0.0)
                        //                 .width(swapchain.get_extent().width as f32)
                        //                 .height(swapchain.get_extent().height as f32)
                        //                 .min_depth(0.0)
                        //                 .max_depth(1.0);
                        //
                        //             let scissor = vk::Rect2D::builder()
                        //                 .offset(vk::Offset2D{x: 0, y: 0})
                        //                 .extent(swapchain.get_extent());
                        //
                        //             let render_pass_begin = vk::RenderPassBeginInfo::builder()
                        //                 .render_pass(render_pass)
                        //                 .framebuffer(framebuffer[0])
                        //                 .render_area(vk::Rect2D::builder()
                        //                     .offset(vk::Offset2D{x: 0, y: 0})
                        //                     .extent(swapchain.get_extent())
                        //                     .build())
                        //                 .clear_values(std::slice::from_ref(&clear_value));
                        //
                        //             render_context.get_device().cmd_set_viewport(
                        //                 command_buffer,
                        //                 0,
                        //                 std::slice::from_ref(&viewport));
                        //
                        //             render_context.get_device().cmd_set_scissor(
                        //                 command_buffer,
                        //                 0,
                        //                 std::slice::from_ref(&scissor));
                        //
                        //             render_context.get_device().cmd_begin_render_pass(
                        //                 command_buffer,
                        //                 &render_pass_begin,
                        //                 vk::SubpassContents::INLINE);
                        //
                        //             render_context.get_device().cmd_bind_pipeline(
                        //                 command_buffer,
                        //                 vk::PipelineBindPoint::GRAPHICS,
                        //                 graphics_pipelines[0]);
                        //         }
                        //
                        //         unsafe {
                        //             let device = render_context.get_device();
                        //             device.update_descriptor_sets(
                        //                 std::slice::from_ref(&descriptor_write),
                        //                 &[]);
                        //             device.cmd_bind_descriptor_sets(
                        //                 command_buffer,
                        //                 vk::PipelineBindPoint::GRAPHICS,
                        //                 pipeline_layout,
                        //                 0,
                        //                 &descriptor_sets,
                        //                 &[]);
                        //             device.cmd_draw(command_buffer, 6, 1, 0, 0);
                        //         }
                        //
                        //         unsafe {
                        //             render_context.get_device().cmd_end_render_pass(command_buffer);
                        //         }
                        //     },
                        //     _ => {
                        //         panic!("This shouldn't have happened");
                        //     }
                        // }
                    }
            ))
            .build()
            .expect("Failed to create transient input PassNode");

        TransientInputPass {
            pass_node
        }
    }
}