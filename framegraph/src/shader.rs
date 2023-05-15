use std::fs;
use std::collections::HashMap;

use ash::vk;
use spirv_reflect::types::descriptor::{ReflectDescriptorType};

use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;

#[derive(Clone)]
pub struct ShaderModule
{
    pub shader: vk::ShaderModule,
    pub descriptor_bindings: HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>>
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

fn create_shader_module(render_context: &VulkanRenderContext, file_name: &str) -> ShaderModule
{
    let (reflection_module, shader) = {
        let bytes = fs::read(file_name)
            .expect(&format!("Unable to load shader at {}", file_name));
        let reflection_module = spirv_reflect::ShaderModule::load_u8_data(&bytes)
            .expect(&format!("Failed to parse shader for reflection data at {}", file_name));
        // let bytes32 = convert_vec8_to_vec32(bytes);
        // assert!(bytes.len() % 4 == 0);

        let create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::ShaderModuleCreateFlags::empty(),
            code_size: bytes.len(),
            p_code: bytes.as_ptr() as *const u32
        };

        // let create_info = vk::ShaderModuleCreateInfo::builder()
        //     .code(&bytes32);
        let shader = unsafe {
            render_context.get_device().borrow().get().create_shader_module(&create_info, None)
                .expect("Failed to create shader")
        };

        (reflection_module, shader)
    };

    // TODO: Add support for compute descriptor set bindings (could just use VK_SHADER_STAGE_ALL)
    // TODO: Add support for immutable samplers
    let mut binding_map : HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>> = HashMap::new();
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
                        .stage_flags(vk::ShaderStageFlags::empty())
                        .descriptor_count(binding_reflect.count)
                        .descriptor_type(translate_descriptor_type(binding_reflect.descriptor_type))
                        .build()
                );
            }

            binding_map.insert(set.set, descriptor_set_bindings);
        }
    }

    ShaderModule::new(shader, binding_map)
}

impl ShaderModule
{
    pub fn new(
        shader: vk::ShaderModule,
        descriptor_bindings: HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>>) -> ShaderModule
    {
        ShaderModule {
            shader,
            descriptor_bindings
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

    pub fn load_shader(&mut self, render_context: &VulkanRenderContext, file_name: &str) -> ShaderModule
    {
        // TODO: can this return a &ShaderModule without a double mutable borrow error in PipelineManager::create_pipeline?
        //let full_path = concat!(concat!(env!("OUT_DIR"), "/shaders/"), file_name);
        // let mut full_path = std::path::PathBuf::from(env!("OUT_DIR"));
        let mut full_path = std::env::current_dir().expect("Couldn't get current directory");
        full_path.push("shaders");
        full_path.push(file_name);
        // full_path.push(file_name);
        let full_name = full_path.display().to_string();
        // let full_path = concat!(concat!(&cd, "/shaders/"), file_name);
        let val = self.shader_cache.get(&full_name);
        match val {
            Some(sm) => {sm.clone()},
            None => {
                let sm = create_shader_module(render_context, &full_name);
                self.shader_cache.insert(full_name, sm.clone());
                sm
            }
        }
    }
}