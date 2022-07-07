use core::ffi::c_void;

use ash::vk;

use crate::context::render_context::RenderContext;
use crate::framegraph::pass_node::{PassNodeBuilder, PassNode};
use crate::resource::resource_manager::{ResourceType, ResourceHandle, ResolvedResource, ResolvedResourceMap, TransientResource};

use untitled::{
    utility,
    utility::constants::*,
    utility::debug::*,
    utility::share,
};
use crate::TransientInputPass;

pub struct OffsetUBO {
    pub offset: [f32; 3]
}

pub struct UBOPass {
    pub pass_node: PassNode,
    pub render_target: ResourceHandle
}

impl Drop for UBOPass {
    fn drop(&mut self) {

    }
}

impl UBOPass {
    fn generate_renderpass(render_context: &mut RenderContext) -> vk::RenderPass {
        let color_attachment = vk::AttachmentDescription::builder()
            .format(vk::Format::R8G8B8A8_SRGB)
            .flags(vk::AttachmentDescriptionFlags::empty())
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        let color_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        // TODO: update vulkan so I can ignore subpasses
        let subpass = vk::SubpassDescription::builder()
            .color_attachments(std::slice::from_ref(&color_attachment_ref))
            .flags(vk::SubpassDescriptionFlags::empty())
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

        let subpass_dependency = vk::SubpassDependency::builder()
            // .src_subpass(vk::SUBPASS_EXTERNAL)
            .src_subpass(0)
            // .dst_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_access_mask(vk::AccessFlags::NONE)
            // .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::MEMORY_WRITE)
            // .dst_access_mask(vk::AccessFlags::SHADER_READ)
            // .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            // .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_stage_mask(vk::PipelineStageFlags::TOP_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dependency_flags(vk::DependencyFlags::empty());

        let render_pass_create_info = vk::RenderPassCreateInfo::builder()
            .flags(vk::RenderPassCreateFlags::empty())
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&subpass_dependency));

        let render_pass = unsafe {
            render_context.get_device().create_render_pass(&render_pass_create_info, None)
                .expect("Failed to create renderpass for UBO Pass")
        };

        render_pass
    }

    pub fn new(render_context: &mut RenderContext) -> Self {
        let render_pass = Self::generate_renderpass(render_context);
        let ubo_create_info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            size: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let uniform_buffer = render_context.create_buffer_persistent(&ubo_create_info);
        let ubo_value = OffsetUBO {
            offset: [0.2, 0.1, 0.0]
        };

        render_context.update_buffer_persistent(&uniform_buffer, |mapped_memory: *mut c_void| {
            println!("Updating uniform buffer");
            unsafe {
                // *mapped_memory as OffsetUBO = ubo_value;
                core::ptr::copy_nonoverlapping(
                    &ubo_value,
                    mapped_memory as *mut OffsetUBO,
                    std::mem::size_of::<OffsetUBO>());
            };
        });

        let ubo_binding = vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::ALL_GRAPHICS,
            p_immutable_samplers: std::ptr::null()
        };
        let ubo_bindings = [ubo_binding];
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            binding_count: ubo_bindings.len() as u32,
            p_bindings: ubo_bindings.as_ptr()
        };
        let descriptor_set_layouts = unsafe {[
            render_context.get_device().create_descriptor_set_layout(
                &descriptor_set_layout_create_info,
                None)
                .expect("Failed to create descriptor set layout")
        ]};
        let descriptor_sets = render_context.create_descriptor_sets(&descriptor_set_layouts);

        let vert_shader_module = share::create_shader_module(
            render_context.get_device(),
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/hello-vert.spv")).to_vec()
        );
        let frag_shader_module = share::create_shader_module(
            render_context.get_device(),
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/hello-frag.spv")).to_vec()
        );
        let main_function_name = std::ffi::CString::new("main").unwrap();
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo {
                // Vertex Shader
                s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                module: vert_shader_module,
                p_name: main_function_name.as_ptr(),
                p_specialization_info: std::ptr::null(),
                stage: vk::ShaderStageFlags::VERTEX,
            },
            vk::PipelineShaderStageCreateInfo {
                // Fragment Shader
                s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                module: frag_shader_module,
                p_name: main_function_name.as_ptr(),
                p_specialization_info: std::ptr::null(),
                stage: vk::ShaderStageFlags::FRAGMENT,
            },
        ];

        let vertex_input_state_create_info = vk::PipelineVertexInputStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineVertexInputStateCreateFlags::empty(),
            vertex_attribute_description_count: 0,
            p_vertex_attribute_descriptions: std::ptr::null(),
            vertex_binding_description_count: 0,
            p_vertex_binding_descriptions: std::ptr::null(),
        };
        let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
            flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
            p_next: std::ptr::null(),
            primitive_restart_enable: vk::FALSE,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        };

        let rasterization_statue_create_info = vk::PipelineRasterizationStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineRasterizationStateCreateFlags::empty(),
            depth_clamp_enable: vk::FALSE,
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::CLOCKWISE,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            rasterizer_discard_enable: vk::FALSE,
            depth_bias_clamp: 0.0,
            depth_bias_constant_factor: 0.0,
            depth_bias_enable: vk::FALSE,
            depth_bias_slope_factor: 0.0,
        };
        let multisample_state_create_info = vk::PipelineMultisampleStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
            flags: vk::PipelineMultisampleStateCreateFlags::empty(),
            p_next: std::ptr::null(),
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            sample_shading_enable: vk::FALSE,
            min_sample_shading: 0.0,
            p_sample_mask: std::ptr::null(),
            alpha_to_one_enable: vk::FALSE,
            alpha_to_coverage_enable: vk::FALSE,
        };

        let stencil_state = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        };

        let depth_state_create_info = vk::PipelineDepthStencilStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
            depth_test_enable: vk::FALSE,
            depth_write_enable: vk::FALSE,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            depth_bounds_test_enable: vk::FALSE,
            stencil_test_enable: vk::FALSE,
            front: stencil_state,
            back: stencil_state,
            max_depth_bounds: 1.0,
            min_depth_bounds: 0.0,
        };

        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::FALSE,
            // color_write_mask: vk::ColorComponentFlags::all(),
            color_write_mask: vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B | vk::ColorComponentFlags::A,
            src_color_blend_factor: vk::BlendFactor::ONE,
            dst_color_blend_factor: vk::BlendFactor::ZERO,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
        }];

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineColorBlendStateCreateFlags::empty(),
            logic_op_enable: vk::FALSE,
            logic_op: vk::LogicOp::COPY,
            attachment_count: color_blend_attachment_states.len() as u32,
            p_attachments: color_blend_attachment_states.as_ptr(),
            blend_constants: [0.0, 0.0, 0.0, 0.0],
        };

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_DYNAMIC_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineDynamicStateCreateFlags::empty(),
            dynamic_state_count: dynamic_states.len() as u32,
            p_dynamic_states: dynamic_states.as_ptr()
        };
        
        let viewport_state = vk::PipelineViewportStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineViewportStateCreateFlags::empty(),
            viewport_count: 1,
            p_viewports: std::ptr::null(),
            scissor_count: 1,
            p_scissors: std::ptr::null()
        };

        let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineLayoutCreateFlags::empty(),
            set_layout_count: descriptor_set_layouts.len() as u32,
            p_set_layouts: descriptor_set_layouts.as_ptr(),
            push_constant_range_count: 0,
            p_push_constant_ranges: std::ptr::null(),
        };

        let pipeline_layout = unsafe {
            render_context.get_device().create_pipeline_layout(
                &pipeline_layout_create_info,
                None
            ).expect("Failed to create pipeline layout for UBOPass")
        };

        let graphics_pipeline_create_infos = [vk::GraphicsPipelineCreateInfo { s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineCreateFlags::empty(),
            stage_count: shader_stages.len() as u32,
            p_stages: shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_input_state_create_info,
            p_input_assembly_state: &vertex_input_assembly_state_info,
            p_tessellation_state: std::ptr::null(),
            // p_viewport_state: &viewport_state_create_info,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterization_statue_create_info,
            p_multisample_state: &multisample_state_create_info,
            p_depth_stencil_state: &depth_state_create_info,
            p_color_blend_state: &color_blend_state,
            p_dynamic_state: &dynamic_state,
            layout: pipeline_layout,
            render_pass,
            subpass: 0,
            base_pipeline_handle: vk::Pipeline::null(),
            base_pipeline_index: -1,
        }];

        let graphics_pipelines = unsafe {
            render_context.get_device().create_graphics_pipelines(
                vk::PipelineCache::null(),
                &graphics_pipeline_create_infos,
                None
            ).expect("Failed to create graphics pipeline for UBOPass")
        };

        unsafe {
            render_context.get_device().destroy_shader_module(vert_shader_module, None);
            render_context.get_device().destroy_shader_module(frag_shader_module, None);
        }

        // let render_target = TransientResource
        let render_target = render_context.create_transient_image(
            vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_SRGB)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                // .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .extent(vk::Extent3D::builder().height(100).width(100).depth(1).build())
                .mip_levels(1)
                .array_layers(1)
                .build()
        );

        // let pass_node = PassNode::builder()
        let pass_node = PassNode::builder()
            .renderpass(render_pass)
            .layout(pipeline_layout)
            .pipeline(graphics_pipelines[0])
            .read(vec![uniform_buffer])
            .write(vec![render_target])
            .fill_commands(Box::new(
                move |render_ctx: &RenderContext,
                      command_buffer: vk::CommandBuffer,
                      inputs: &ResolvedResourceMap,
                      outputs: &ResolvedResourceMap|
                    {
                        println!("I'm doing something!");
                        let render_target = outputs.get(&render_target)
                            .expect("No resolved render target");
                        let ubo = inputs.get(&uniform_buffer)
                            .expect("No resolved UBO");
                        match (&render_target.resource, &ubo.resource) {
                            (ResourceType::Image(rt), ResourceType::Buffer(buffer)) => {
                                let framebuffer = render_ctx.create_framebuffers(
                                    render_pass,
                                    &vk::Extent2D::builder().width(100).height(100),
                                    std::slice::from_ref(&rt)
                                );

                                let descriptor_buffer = vk::DescriptorBufferInfo {
                                    buffer: *buffer,
                                    offset: 0,
                                    range: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize
                                };

                                let descriptor_write = vk::WriteDescriptorSet {
                                    s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                                    p_next: std::ptr::null(),
                                    dst_set: descriptor_sets[0],
                                    dst_binding: 0,
                                    dst_array_element: 0,
                                    descriptor_count: 1,
                                    descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                                    p_buffer_info: &descriptor_buffer,
                                    ..Default::default()
                                };
                                let descriptor_write_sets = [descriptor_write];

                                // TODO: move this into framegraph? Not sure how to bind framebuffer first?
                                // begin renderpass
                                unsafe {
                                    let clear_value = vk::ClearValue {
                                        color: vk::ClearColorValue {
                                            float32: [0.1, 0.1, 0.1, 1.0]
                                        }
                                    };

                                    let viewport = vk::Viewport::builder()
                                        .x(0.0)
                                        .y(0.0)
                                        .width(100.0)
                                        .height(100.0)
                                        .min_depth(0.0)
                                        .max_depth(1.0)
                                        .build();

                                    let scissor = vk::Rect2D::builder()
                                        .offset(vk::Offset2D{x: 0, y: 0})
                                        .extent(vk::Extent2D::builder().width(100).height(100).build())
                                        .build();

                                    let render_pass_begin = vk::RenderPassBeginInfo::builder()
                                        .render_pass(render_pass)
                                        .framebuffer(framebuffer[0])
                                        .render_area(vk::Rect2D::builder()
                                                         .offset(vk::Offset2D{x: 0, y: 0})
                                                         .extent(vk::Extent2D::builder().width(100).height(100).build())
                                                         .build())
                                        .clear_values(std::slice::from_ref(&clear_value));

                                    render_ctx.get_device().cmd_set_viewport(
                                        command_buffer,
                                        0,
                                        std::slice::from_ref(&viewport));

                                    render_ctx.get_device().cmd_set_scissor(
                                        command_buffer,
                                        0,
                                        std::slice::from_ref(&scissor));

                                    render_ctx.get_device().cmd_begin_render_pass(
                                        command_buffer,
                                        &render_pass_begin,
                                        vk::SubpassContents::INLINE);

                                    render_ctx.get_device().cmd_bind_pipeline(
                                        command_buffer,
                                        vk::PipelineBindPoint::GRAPHICS,
                                        graphics_pipelines[0])
                                }

                                // Draw calls
                                unsafe {
                                    let device = render_ctx.get_device();
                                    device.update_descriptor_sets(&descriptor_write_sets, &[]);
                                    device.cmd_bind_descriptor_sets(
                                        command_buffer,
                                        vk::PipelineBindPoint::GRAPHICS,
                                        pipeline_layout,
                                        0,
                                        &descriptor_sets,
                                        &[]);
                                    device.cmd_draw(command_buffer, 3, 1, 0, 0);
                                }

                                // TODO: move this to framegraph?
                                // End renderpass
                                unsafe {
                                    let graphics_queue_index = render_ctx.get_graphics_queue_index();
                                    render_ctx.get_device().cmd_end_render_pass(command_buffer);
                                    // let image_transition = vk::ImageMemoryBarrier::builder()
                                    //     .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                                    //     .dst_access_mask(vk::AccessFlags::SHADER_READ)
                                    //     .image(rt.image)
                                    //     .old_layout(vk::ImageLayout::UNDEFINED)
                                    //     .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                    //     .src_queue_family_index(graphics_queue_index)
                                    //     .dst_queue_family_index(graphics_queue_index)
                                    //     .subresource_range(vk::ImageSubresourceRange::builder()
                                    //         .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    //         .base_mip_level(0)
                                    //         .level_count(1)
                                    //         .base_array_layer(0)
                                    //         .layer_count(1)
                                    //         .build());
                                    // render_ctx.get_device().cmd_pipeline_barrier(
                                    //     command_buffer,
                                    //     vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                    //     vk::PipelineStageFlags::FRAGMENT_SHADER,
                                    //     vk::DependencyFlags::empty(),
                                    //     &[],
                                    //     &[],
                                    //     std::slice::from_ref(&image_transition));
                                }
                            },
                            _ => {}
                        }
                }
            ))
            .build()
            .expect("Failed to create PassNode");

        UBOPass {
            pass_node,
            render_target
        }
    }
}