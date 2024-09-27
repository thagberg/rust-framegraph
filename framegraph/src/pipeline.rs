use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle;
use api_types::device::{DevicePipeline, DeviceWrapper};
use context::render_context::RenderContext;

use crate::shader::{Shader, ShaderManager};

extern crate context;
use context::vulkan_render_context::VulkanRenderContext;
use profiling::enter_span;

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

pub struct PipelineDescription
{
    vertex_input: vk::PipelineVertexInputStateCreateInfo,
    dynamic_states: Vec<vk::DynamicState>,
    rasterization: RasterizationType,
    depth_stencil: DepthStencilType,
    blend: BlendType,
    name: String,
    vertex_shader: Rc<RefCell<Shader>>,
    fragment_shader: Rc<RefCell<Shader>>
}

impl Hash for PipelineDescription
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        // TODO: this is an inadequate hash
        // will need to actually use some pipeline state for a better hash
        self.name.hash(state);
    }
}

impl PipelineDescription
{
    pub fn new(
        vertex_input: vk::PipelineVertexInputStateCreateInfo,
        dynamic_states: Vec<vk::DynamicState>,
        rasterization: RasterizationType,
        depth_stencil: DepthStencilType,
        blend: BlendType,
        name: &str,
        vertex_shader: Rc<RefCell<Shader>>,
        fragment_shader: Rc<RefCell<Shader>>) -> Self
    {
        PipelineDescription {
            vertex_input,
            dynamic_states,
            rasterization,
            depth_stencil,
            blend,
            name: name.to_string(),
            vertex_shader,
            fragment_shader
        }
    }

    pub fn get_name(&self) -> &str { &self.name }
}


#[derive(Debug)]
pub struct ComputePipelineDescription
{
    compute_name: String
}

impl Hash for ComputePipelineDescription
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.compute_name.hash(state);
    }
}

impl ComputePipelineDescription {
    pub fn new(
        compute_name: &str
    ) -> Self {
        ComputePipelineDescription {
            compute_name: compute_name.to_string()
        }
    }
}

#[derive(Clone)]
pub struct Pipeline
{
    pub device_pipeline: DevicePipeline
}

impl Debug for Pipeline {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("device pipeline", &self.device_pipeline.pipeline.as_raw())
            .finish()
    }
}

impl Pipeline {
    pub fn new(device_pipeline: DevicePipeline) -> Pipeline
    {
        Pipeline {
            device_pipeline
        }
    }

    pub fn get_pipeline(&self) -> vk::Pipeline {
        self.device_pipeline.pipeline
    }

    pub fn get_pipeline_layout(&self) -> vk::PipelineLayout {
        self.device_pipeline.pipeline_layout
    }
}

#[derive(Debug)]
pub struct VulkanPipelineManager
{
    pipeline_cache: HashMap<u64, Arc<Mutex<Pipeline>>>,
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
                cull_mode: vk::CullModeFlags::NONE,
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

fn generate_blend_attachments(blend_type: BlendType) -> [vk::PipelineColorBlendAttachmentState; 1] {
    match blend_type
    {
        BlendType::None => {
            // let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            //     blend_enable: vk::FALSE,
            //     // color_write_mask: vk::ColorComponentFlags::all(),
            //     color_write_mask: vk::ColorComponentFlags::R | vk::ColorComponentFlags::G | vk::ColorComponentFlags::B | vk::ColorComponentFlags::A,
            //     src_color_blend_factor: vk::BlendFactor::ONE,
            //     dst_color_blend_factor: vk::BlendFactor::ZERO,
            //     color_blend_op: vk::BlendOp::ADD,
            //     src_alpha_blend_factor: vk::BlendFactor::ONE,
            //     dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            //     alpha_blend_op: vk::BlendOp::ADD,
            // }];
            [vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(false)
                .color_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .build()]
        },
        BlendType::Transparent => {
            [vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(true)
                .color_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
                .build()]
        }
        _ => {
            panic!("Need to implement the rest of the blend states")
        }
    }
}

fn generate_blend_state(blend_type: BlendType, attachments: &[vk::PipelineColorBlendAttachmentState]) -> vk::PipelineColorBlendStateCreateInfo
{
    match blend_type
    {
        BlendType::None => {
            vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op_enable(false)
                .logic_op(vk::LogicOp::NO_OP)
                .attachments(attachments)
                .blend_constants([0.0, 0.0, 0.0, 0.0])
                .build()
            // vk::PipelineColorBlendStateCreateInfo {
            //     s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
            //     p_next: std::ptr::null(),
            //     flags: vk::PipelineColorBlendStateCreateFlags::empty(),
            //     logic_op_enable: vk::FALSE,
            //     logic_op: vk::LogicOp::COPY,
            //     attachment_count: color_blend_attachment_states.len() as u32,
            //     p_attachments: color_blend_attachment_states.as_ptr(),
            //     blend_constants: [0.0, 0.0, 0.0, 0.0],
            // }
        },
        BlendType::Transparent => {
            vk::PipelineColorBlendStateCreateInfo::builder()
                .attachments(attachments)
                .blend_constants([1.0, 1.0, 1.0, 1.0])
                .build()
        }
        _ => {
            panic!("Need to implement the rest of the blend states")
        }
    }
}

fn create_descriptor_set_layouts(render_context: &VulkanRenderContext, full_bindings: &HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>>) -> Vec<vk::DescriptorSetLayout> {

    let mut descriptor_set_layouts: Vec<vk::DescriptorSetLayout> = Vec::new();

    // first find the highest set
    let highest_set = {
        let mut highest = 0;
        for set in full_bindings.keys() {
            if *set > highest {
                highest = *set;
            }
        }
        highest
    };
    descriptor_set_layouts.resize((highest_set + 1) as  usize, vk::DescriptorSetLayout::null());

    // then fill the DescriptorSetLayout vector, using null layouts to fill the holes
    // e.g. if a pipeline explicitly uses sets 0 and 2, set 1 will be a null handle
    for set in (0..=highest_set) {
        if let Some(bindings) = full_bindings.get(&set) {
            let layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(&bindings)
                .build();

            let layout = unsafe {
                render_context.get_device().lock()
                    .expect("Failed to obtain device lock")
                    .get().create_descriptor_set_layout(
                    &layout_create_info,
                    None)
                    .expect("Failed to create descriptor set layout")
            };
            // assert!((*set as usize) <= descriptor_set_layouts.len(), "Holes in used descriptor sets not allowed");
            descriptor_set_layouts[set as usize] = layout;
        } else {
            descriptor_set_layouts[set as  usize] = vk::DescriptorSetLayout::null();
        }
    }

    descriptor_set_layouts
}


impl VulkanPipelineManager {
    pub fn new() -> VulkanPipelineManager
    {
        VulkanPipelineManager {
            pipeline_cache: HashMap::new(),
            shader_manager: ShaderManager::new()
        }
    }

    pub fn create_compute_pipeline(
        &mut self,
        render_context: &VulkanRenderContext,
        pipeline_description: &ComputePipelineDescription) -> Arc<Mutex<Pipeline>> {

        let mut pipeline_hasher = DefaultHasher::new();
        pipeline_description.hash(&mut pipeline_hasher);
        let pipeline_key = pipeline_hasher.finish();
        let pipeline_val = self.pipeline_cache.get(&pipeline_key);
        match pipeline_val {
            Some(pipeline) => { pipeline.clone() },
            None => {
                let mut compute_shader_module = self.shader_manager.load_shader(
                    render_context.get_device(),
                    &pipeline_description.compute_name);
                let mut compute_shader_ref = compute_shader_module.lock().unwrap();

                let mut full_bindings: HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>> = HashMap::new();
                for (set, bindings) in &mut compute_shader_ref.descriptor_bindings {
                    let set_bindings = full_bindings.entry(*set).or_insert(Vec::new());
                    set_bindings.extend(bindings.iter());
                    for binding in set_bindings {
                        binding.stage_flags = vk::ShaderStageFlags::COMPUTE;
                    }
                }

                let descriptor_set_layouts = create_descriptor_set_layouts(render_context, &full_bindings);

                // let descriptor_sets = render_context.create_descriptor_sets(&descriptor_set_layouts);

                let pipeline = {
                    let pipeline_layout = {
                        let pipeline_layout_create = vk::PipelineLayoutCreateInfo::builder()
                            .set_layouts(&descriptor_set_layouts);
                        unsafe {
                            let device_ref = render_context.get_device().lock()
                                .expect("Failed to obtain device lock");
                            device_ref.get().create_pipeline_layout(&pipeline_layout_create, None)
                                .expect("Failed to create pipeline layout")
                        }
                    };

                    let main_name = std::ffi::CString::new("main").unwrap();
                    let shader_stage = vk::PipelineShaderStageCreateInfo::builder()
                        .module(compute_shader_ref.shader.shader_module.clone())
                        .name(&main_name)
                        .stage(vk::ShaderStageFlags::COMPUTE);

                    let compute_pipeline_info = vk::ComputePipelineCreateInfo::builder()
                        .stage(*shader_stage)
                        .layout(pipeline_layout)
                        .build();

                    let device_pipeline = DeviceWrapper::create_compute_pipeline(
                        render_context.get_device(),
                        &compute_pipeline_info,
                        pipeline_layout,
                        descriptor_set_layouts,
                        &pipeline_description.compute_name);

                    Arc::new(Mutex::new(Pipeline::new(
                        device_pipeline)))
                };

                self.pipeline_cache.insert(pipeline_key, pipeline.clone());
                pipeline
            }
        }
    }

    pub fn create_pipeline(
        &mut self,
        render_context: &VulkanRenderContext,
        render_pass: vk::RenderPass,
        pipeline_description: &PipelineDescription) -> Arc<Mutex<Pipeline>> {
        enter_span!(tracing::Level::TRACE, "Create or fetch Pipeline");

        // TODO: define a PipelineKey type and require the consumer to provide it here
        //  to avoid needing to calculate a hash for each used pipeline each frame?
        let mut pipeline_hasher = DefaultHasher::new();
        pipeline_description.hash(&mut pipeline_hasher);
        let pipeline_key = pipeline_hasher.finish();
        let pipeline_val = self.pipeline_cache.get(&pipeline_key);
        match pipeline_val {
            Some(pipeline) => { pipeline.clone() },
            None => {
                // Need to reconcile descriptor bindings between vertex and fragment stages
                //  i.e. - Could have duplicate bindings for descriptors used in both stages, or
                //  bindings only used in a single stage but are part of a larger descriptor set
                let mut full_bindings: HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>> = HashMap::new();
                for (set, bindings) in &pipeline_description.vertex_shader.borrow().descriptor_bindings {
                    let set_bindings = full_bindings.entry(*set).or_insert(Vec::new());
                    // set_bindings.copy_from_slice(&bindings);
                    set_bindings.extend(bindings.iter());
                    for binding in set_bindings {
                        binding.stage_flags = vk::ShaderStageFlags::VERTEX;
                    }
                }
                for (set, bindings) in &pipeline_description.fragment_shader.borrow().descriptor_bindings {
                    let set_bindings = full_bindings.entry(*set).or_insert(Vec::new());
                    for binding in bindings {
                        let duplicate = set_bindings.iter_mut().find(|x| {
                            x.binding == binding.binding && x.descriptor_count == binding.descriptor_count && x.descriptor_type == binding.descriptor_type
                        });
                        match duplicate {
                            Some(dupe_binding) => {
                               dupe_binding.stage_flags |= vk::ShaderStageFlags::FRAGMENT;
                            },
                            None => {
                                let mut new_binding = binding.clone();
                                new_binding.stage_flags = vk::ShaderStageFlags::FRAGMENT;
                                set_bindings.push(new_binding);
                            }
                        }
                    }
                }

                let descriptor_set_layouts = create_descriptor_set_layouts(render_context, &full_bindings);

                // let descriptor_sets = render_context.create_descriptor_sets(&descriptor_set_layouts);

                let pipeline_layout = {
                        let pipeline_layout_create = vk::PipelineLayoutCreateInfo::builder()
                            .set_layouts(&descriptor_set_layouts);
                        unsafe {
                            render_context.get_device().lock()
                                .expect("Failed to obtain device lock")
                                .get().create_pipeline_layout(&pipeline_layout_create, None)
                                .expect("Failed to create pipeline layout")
                        }
                };

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
                        module: pipeline_description.vertex_shader.borrow().shader.shader_module.clone(),
                        p_name: main_name.as_ptr(),
                        p_specialization_info: std::ptr::null(),
                        stage: vk::ShaderStageFlags::VERTEX,
                    },
                    vk::PipelineShaderStageCreateInfo {
                        // Fragment Shader
                        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                        p_next: std::ptr::null(),
                        flags: vk::PipelineShaderStageCreateFlags::empty(),
                        module: pipeline_description.fragment_shader.borrow().shader.shader_module.clone(),
                        p_name: main_name.as_ptr(),
                        p_specialization_info: std::ptr::null(),
                        stage: vk::ShaderStageFlags::FRAGMENT,
                    },
                ];

                let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
                    .dynamic_states(&pipeline_description.dynamic_states);

                let rasterization_state = generate_rasteration_state(pipeline_description.rasterization);
                let depth_stencil_state = generate_depth_stencil_state(pipeline_description.depth_stencil);
                let blend_attachments = generate_blend_attachments(pipeline_description.blend);
                let blend_state = generate_blend_state(pipeline_description.blend, &blend_attachments);

                let graphics_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
                    .stages(&shader_stages)
                    .input_assembly_state(&vertex_input_assembly_state_info)
                    .vertex_input_state(&pipeline_description.vertex_input)
                    .viewport_state(&viewport_state)
                    .rasterization_state(&rasterization_state)
                    .multisample_state(&multisample_state_create_info)
                    .depth_stencil_state(&depth_stencil_state)
                    .color_blend_state(&blend_state)
                    // .dynamic_state(&pipeline_description.dynamic_state)
                    .dynamic_state(&dynamic_state)
                    // .layout(frag_shader_module.pipeline_layout)
                    .layout(pipeline_layout)
                    .render_pass(render_pass)
                    .subpass(0); // TODO: this shouldn't be static
                // .build();

                let device_pipeline = DeviceWrapper::create_pipeline(
                    render_context.get_device(),
                    &graphics_pipeline_info,
                    pipeline_layout,
                    descriptor_set_layouts,
                    pipeline_description.get_name());
                let pipeline = Arc::new(Mutex::new(Pipeline::new(
                    device_pipeline)));
                self.pipeline_cache.insert(pipeline_key, pipeline.clone());
                pipeline
            }
        }
    }
}