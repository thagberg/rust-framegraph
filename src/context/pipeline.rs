use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use ash::vk;

use crate::context::shader::{ShaderManager, ShaderModule};
use crate::context::render_context::RenderContext;

#[derive(Copy, Clone)]
pub enum BlendType
{
    None,
    Alpha,
    Transparent
}

#[derive(Copy, Clone)]
pub enum DepthStencilType
{
    Disable,
    Enable
}

#[derive(Copy, Clone)]
pub enum RasterizationType
{
    Standard
}

// #[derive(Hash)]
pub struct PipelineDescription
{
    vertex_input: vk::PipelineVertexInputStateCreateInfo,
    dynamic_state: vk::PipelineDynamicStateCreateInfo,
    rasterization: RasterizationType,
    depth_stencil: DepthStencilType,
    blend: BlendType,
    vertex_name: String,
    fragment_name: String
}

impl Hash for PipelineDescription
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        // TODO: this is an inadequate hash
        // will need to actually use some pipeline state for a better hash
        self.vertex_name.hash(state);
        self.fragment_name.hash(state);
    }
}

pub struct Pipeline
{
    graphics_pipeline: vk::Pipeline
}

pub struct PipelineManager
{
    pipeline_cache: HashMap<u64, Pipeline>,
    shader_manager: ShaderManager
}

const STENCIL_STATE_KEEP: vk::StencilOpState = vk::StencilOpState {
    fail_op: vk::StencilOp::KEEP,
    pass_op: vk::StencilOp::KEEP,
    depth_fail_op: vk::StencilOp::KEEP,
    compare_op: vk::CompareOp::ALWAYS,
    compare_mask: 0,
    write_mask: 0,
    reference: 0,
};

fn generate_rasteration_state(rasterization_type: RasterizationType) -> vk::PipelineRasterizationStateCreateInfo
{
    match rasterization_type
    {
        RasterizationType::Standard => {
            vk::PipelineRasterizationStateCreateInfo
            {
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
            }
        },
        _ => {
            panic!("Invalid Rasterization Type")
        }
    }
}

fn generate_depth_stencil_state(depth_stencil_type: DepthStencilType) -> vk::PipelineDepthStencilStateCreateInfo
{
    match depth_stencil_type
    {
        DepthStencilType::Enable => {
            vk::PipelineDepthStencilStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
                depth_test_enable: vk::TRUE,
                depth_write_enable: vk::TRUE,
                depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
                depth_bounds_test_enable: vk::FALSE,
                stencil_test_enable: vk::FALSE,
                front: STENCIL_STATE_KEEP,
                back: STENCIL_STATE_KEEP,
                max_depth_bounds: 1.0,
                min_depth_bounds: 0.0,
            }
        },
        _ => {
            vk::PipelineDepthStencilStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
                depth_test_enable: vk::FALSE,
                depth_write_enable: vk::FALSE,
                depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
                depth_bounds_test_enable: vk::FALSE,
                stencil_test_enable: vk::FALSE,
                front: STENCIL_STATE_KEEP,
                back: STENCIL_STATE_KEEP,
                max_depth_bounds: 1.0,
                min_depth_bounds: 0.0
            }
        }
    }
}

fn generate_blend_state(blend_type: BlendType) -> vk::PipelineColorBlendStateCreateInfo
{
    match blend_type
    {
        BlendType::None => {
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
            vk::PipelineColorBlendStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: vk::PipelineColorBlendStateCreateFlags::empty(),
                logic_op_enable: vk::FALSE,
                logic_op: vk::LogicOp::COPY,
                attachment_count: color_blend_attachment_states.len() as u32,
                p_attachments: color_blend_attachment_states.as_ptr(),
                blend_constants: [0.0, 0.0, 0.0, 0.0],
            }
        },
        _ => {
            panic!("Need to implement the rest of the blend states")
        }
    }
}

impl Pipeline
{
    pub fn new(graphics_pipeline: vk::Pipeline) -> Pipeline
    {
        Pipeline {
            graphics_pipeline
        }
    }
}

impl PipelineDescription
{
    pub fn new(
        vertex_input: vk::PipelineVertexInputStateCreateInfo,
        dynamic_state: vk::PipelineDynamicStateCreateInfo,
        rasterization: RasterizationType,
        depth_stencil: DepthStencilType,
        blend: BlendType,
        vertex_name: &str,
        fragment_name: &str) -> PipelineDescription
    {
        PipelineDescription {
            vertex_input,
            dynamic_state,
            rasterization,
            depth_stencil,
            blend,
            vertex_name: vertex_name.to_string(),
            fragment_name: fragment_name.to_string()
        }
    }
}

impl PipelineManager
{
    pub fn new() -> PipelineManager
    {
        PipelineManager {
            pipeline_cache: HashMap::new(),
            shader_manager: ShaderManager::new()
        }
    }

    pub fn create_pipeline(
        &mut self,
        render_context: &RenderContext,
        render_pass: vk::RenderPass,
        pipeline_description: &PipelineDescription) -> &Pipeline
    {
        // TODO: define a PipelineKey type and require the consumer to provide it here
        //  to avoid needing to calculate a hash for each used pipeline each frame?
        let mut pipeline_hasher = DefaultHasher::new();
        pipeline_description.hash(&mut pipeline_hasher);
        let pipeline_key = pipeline_hasher.finish();
        self.pipeline_cache.entry(pipeline_key).or_insert(
    {
                // if self.pipeline_cache.contains_key(&pipeline_key)
                let vertex_shader_module = self.shader_manager.load_shader(
                    render_context,
                    &pipeline_description.vertex_name);
                let frag_shader_module = self.shader_manager.load_shader(
                    render_context,
                    &pipeline_description.fragment_name);
                let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
                    flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
                    p_next: std::ptr::null(),
                    primitive_restart_enable: vk::FALSE,
                    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                };

                // TODO: parameterize multisample state
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

                let viewport_state = vk::PipelineViewportStateCreateInfo {
                    s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
                    p_next: std::ptr::null(),
                    flags: vk::PipelineViewportStateCreateFlags::empty(),
                    viewport_count: 1,
                    p_viewports: std::ptr::null(),
                    scissor_count: 1,
                    p_scissors: std::ptr::null()
                };

                let main_name = std::ffi::CString::new("main").unwrap();
                let shader_stages = [
                    vk::PipelineShaderStageCreateInfo {
                        // Vertex Shader
                        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                        p_next: std::ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        module: vertex_shader_module.shader.clone(),
                        p_name: main_name.as_ptr(),
                        p_specialization_info: std::ptr::null(),
                        stage: vk::ShaderStageFlags::VERTEX,
                    },
                    vk::PipelineShaderStageCreateInfo {
                        // Fragment Shader
                        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                        p_next: std::ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        module: frag_shader_module.shader.clone(),
                        p_name: main_name.as_ptr(),
                        p_specialization_info: std::ptr::null(),
                        stage: vk::ShaderStageFlags::FRAGMENT,
                    },
                ];

                let graphics_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
                    .stages(&shader_stages)
                    .input_assembly_state(&vertex_input_assembly_state_info)
                    .vertex_input_state(&pipeline_description.vertex_input)
                    .viewport_state(&viewport_state)
                    .rasterization_state(&generate_rasteration_state(pipeline_description.rasterization))
                    .multisample_state(&multisample_state_create_info)
                    .depth_stencil_state(&generate_depth_stencil_state(pipeline_description.depth_stencil))
                    .color_blend_state(&generate_blend_state(pipeline_description.blend))
                    .dynamic_state(&pipeline_description.dynamic_state)
                    .layout(frag_shader_module.pipeline_layout)
                    .render_pass(render_pass)
                    .subpass(0) // TODO: this shouldn't be static
                    .build();

                let graphics_pipeline = unsafe {
                    render_context.get_device().create_graphics_pipelines(
                        vk::PipelineCache::null(),
                        std::slice::from_ref(&graphics_pipeline_info),
                        None
                    ).expect("Failed to create Graphics Pipeline")
                };
                Pipeline::new(graphics_pipeline[0])
            }
        )
    }
}