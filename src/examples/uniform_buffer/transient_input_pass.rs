use ash::vk;

use crate::context::render_context::RenderContext;
use crate::framegraph::pass_node::{PassNodeBuilder, PassNode};
use crate::resource::resource_manager::{ResourceType, ResourceHandle, ResolvedResource, TransientResourceMap};

use untitled::{
    utility::share,
};

pub struct TransientInputPass {
    // pub pass_node: PassNode
}

impl TransientInputPass {
    pub fn new(render_context: &mut RenderContext, render_pass: vk::RenderPass) -> Self {
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
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/transient_input-vert.spv")).to_vec()
        );
        let frag_shader_module = share::create_shader_module(
            render_context.get_device(),
            include_bytes!(concat!(env!("OUT_DIR"), "/shaders/transient_input-frag.spv")).to_vec()
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
            .renderpass(render_pass)
            .layout(pipeline_layout)
            .pipeline(graphics_pipelines[0])
            .inputs(vec![])
            .fill_commands(Box::new(move |render_context: &RenderContext, command_buffer: vk::CommandBuffer, inputs: &TransientResourceMap| {
            }))
            .build()
            .expect("Failed ot create transient input PassNode");

        TransientInputPass {

        }
    }
}