// use std::fs;
use std::collections::HashMap;
use ash::vk;
use spirv_reflect::ShaderModule as ReflectShaderModule;
use spirv_reflect::types::descriptor::{ReflectDescriptorType};

use crate::context::render_context::RenderContext;

pub struct ShaderModule
{
    shader: vk::ShaderModule,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    descriptor_sets: Vec<vk::DescriptorSet>,
    pipeline_layout: vk::PipelineLayout
}

pub struct ShaderManager
{
    shader_cache: HashMap<str, ShaderModule>
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

fn create_shader_module(render_context: &RenderContext, file_name: &str) -> ShaderModule
{
    let reflection_module = spirv_reflect::ShaderModule::load_u8_data(&bytes)
        .expect(&format!("Failed to parse shader for reflection data at {}", file_name));
    // let bytes = fs::read(file_name)
    //     .expect(&format!("Unable to load shader at {}", file_name));
    let bytes = reflection_module.get_code();

    let create_info = vk::ShaderModuleCreateInfo::builder()
        .code(&bytes);
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

    ShaderModule::new()
}

impl ShaderModule
{
    pub fn new() -> ShaderModule
    {
        ShaderModule {
            shader: vk::ShaderModule::null(),
            descriptor_set_layouts: Vec::new(),
            descriptor_sets: Vec::new(),
            pipeline_layout: vk::PipelineLayout::null()
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

    pub fn load_shader(&mut self, render_context: &RenderContext, file_name: &str) -> &ShaderModule
    {
        let found = self.shader_cache.get(file_name);
        match found
        {
            Some(shader) => {
               shader
            },
            _ => {

            }
        }
    }
}