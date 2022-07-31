use std::fs;
use std::collections::HashMap;
use ash::vk;
use spirv_reflect::types::descriptor::{ReflectDescriptorType};

use context::render_context::RenderContext;

#[derive(Clone)]
pub struct ShaderModule
{
    pub shader: vk::ShaderModule,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub pipeline_layout: vk::PipelineLayout
}

pub struct ShaderManager
{
    shader_cache: HashMap<String, ShaderModule>
}

fn translate_descriptor_type(reflect_type: ReflectDescriptorType) -> vk::DescriptorType
{
    match reflect_type
    {
        ReflectDescriptorType::Sampler => vk::DescriptorType::SAMPLER,
        ReflectDescriptorType::CombinedImageSampler => vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        ReflectDescriptorType::SampledImage => vk::DescriptorType::SAMPLED_IMAGE,
        ReflectDescriptorType::StorageImage => vk::DescriptorType::STORAGE_IMAGE,
        ReflectDescriptorType::UniformTexelBuffer => vk::DescriptorType::UNIFORM_TEXEL_BUFFER,
        ReflectDescriptorType::StorageTexelBuffer => vk::DescriptorType::STORAGE_TEXEL_BUFFER,
        ReflectDescriptorType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
        ReflectDescriptorType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
        ReflectDescriptorType::UniformBufferDynamic => vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
        ReflectDescriptorType::StorageBufferDynamic => vk::DescriptorType::STORAGE_BUFFER_DYNAMIC,
        ReflectDescriptorType::InputAttachment => vk::DescriptorType::INPUT_ATTACHMENT,
        ReflectDescriptorType::AccelerationStructureNV => vk::DescriptorType::ACCELERATION_STRUCTURE_NV,
        ReflectDescriptorType::Undefined => panic!("Invalid descriptor type; can't translate")
    }
}

fn convert_vec8_to_vec32(mut vec8: Vec<u8>) -> Vec<u32>
{
    unsafe {
        vec8.shrink_to_fit();
        let mut len = vec8.len();

        let padding = len % 4;
        for i in 0..padding {
            vec8.push(0);
        }

        let ptr = vec8.as_mut_ptr() as *mut u32;
        let cap = vec8.capacity();
        len = vec8.len();

        Vec::from_raw_parts(ptr, len / 4, cap / 4)
    }
}

fn create_shader_module(render_context: &RenderContext, file_name: &str) -> ShaderModule
{
    let bytes = fs::read(file_name)
        .expect(&format!("Unable to load shader at {}", file_name));
    let reflection_module = spirv_reflect::ShaderModule::load_u8_data(&bytes)
        .expect(&format!("Failed to parse shader for reflection data at {}", file_name));
    let bytes32 = convert_vec8_to_vec32(bytes);

    let create_info = vk::ShaderModuleCreateInfo::builder()
        .code(&bytes32);
    let shader = unsafe {
        render_context.get_device().create_shader_module(&create_info, None)
            .expect("Failed to create shader")
    };

    // TODO: Add support for compute descriptor set bindings (could just use VK_SHADER_STAGE_ALL)
    // TODO: Add support for immutable samplers
    let mut descriptor_set_layouts: Vec<vk::DescriptorSetLayout> = Vec::new();
    if let Ok(descriptor_sets_reflection) = reflection_module.enumerate_descriptor_sets(None)
    {
        for set in descriptor_sets_reflection
        {
            let mut descriptor_set_bindings: Vec<vk::DescriptorSetLayoutBinding> = Vec::new();
            for binding_reflect in set.bindings
            {
                descriptor_set_bindings.push(
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(binding_reflect.binding)
                        .stage_flags(vk::ShaderStageFlags::ALL_GRAPHICS)
                        .descriptor_count(binding_reflect.count)
                        .descriptor_type(translate_descriptor_type(binding_reflect.descriptor_type))
                        .build()
                );
            }

            let layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(&descriptor_set_bindings)
                .build();

            let layout = unsafe {
                render_context.get_device().create_descriptor_set_layout(
                    &layout_create_info,
                    None)
                    .expect("Failed to create descriptor set layout")
            };

            descriptor_set_layouts.push(layout);
        }
    }

    let descriptor_sets = render_context.create_descriptor_sets(&descriptor_set_layouts);

    let pipeline_layout_create = vk::PipelineLayoutCreateInfo::builder()
        .set_layouts(&descriptor_set_layouts);
    let pipeline_layout = unsafe {
        render_context.get_device().create_pipeline_layout(&pipeline_layout_create, None)
            .expect("Failed to create pipeline layout")
    };

    ShaderModule::new(shader, descriptor_set_layouts, descriptor_sets, pipeline_layout)
}

impl ShaderModule
{
    pub fn new(
        shader: vk::ShaderModule,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        descriptor_sets: Vec<vk::DescriptorSet>,
        pipeline_layout: vk::PipelineLayout) -> ShaderModule
    {
        ShaderModule {
            shader,
            descriptor_set_layouts,
            descriptor_sets,
            pipeline_layout
        }
    }
}

impl ShaderManager
{
    pub fn new() -> ShaderManager
    {
        ShaderManager {
           shader_cache: HashMap::new()
        }
    }

    pub fn load_shader(&mut self, render_context: &RenderContext, file_name: &str) -> ShaderModule
    {
        // TODO: can this return a &ShaderModule without a double mutable borrow error in PipelineManager::create_pipeline?
        self.shader_cache.entry(file_name.parse().unwrap()).or_insert(
            create_shader_module(render_context, file_name)
        ).clone()
    }
}