use std::cell::RefCell;
use std::fs;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

use ash::vk;
use rspirv_reflect;
use rspirv_reflect::BindingCount;
use api_types::device::{DeviceShader, DeviceWrapper};

fn create_shader_module(device: Rc<RefCell<DeviceWrapper>>, file_name: &str) -> Shader
{
    let (reflection_module, shader) = {
        let bytes = fs::read(file_name)
            .expect(&format!("Unable to load shader at {}", file_name));

        let reflection_module = rspirv_reflect::Reflection::new_from_spirv(&bytes)
            .expect(&format!("Failed to parse shader for reflection data at {}", file_name));

        let create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::ShaderModuleCreateFlags::empty(),
            code_size: bytes.len(),
            p_code: bytes.as_ptr() as *const u32
        };

        let shader = DeviceWrapper::create_shader(device, file_name, &create_info);

        (reflection_module, shader)
    };

    // TODO: Add support for compute descriptor set bindings (could just use VK_SHADER_STAGE_ALL)
    // TODO: Add support for immutable samplers
    let mut binding_map : HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>> = HashMap::new();
    let descriptor_sets_reflection = reflection_module.get_descriptor_sets()
        .expect("Failed to get descriptor sets for reflected shader");
    for set in descriptor_sets_reflection {
        let mut descriptor_set_bindings: Vec<vk::DescriptorSetLayoutBinding> = Vec::new();
        for binding_reflect in set.1 {
            let binding_count = match binding_reflect.1.binding_count {
                BindingCount::One => { 1 }
                BindingCount::StaticSized(size) => {size}
                BindingCount::Unbounded => {
                    panic!("Unbounded descriptor binding count not supported");
                }
            };

            let descriptor_type = vk::DescriptorType::from_raw(binding_reflect.1.ty.0 as i32);

            descriptor_set_bindings.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding_reflect.0)
                    .stage_flags(vk::ShaderStageFlags::empty())
                    .descriptor_count(binding_count as u32)
                    .descriptor_type(descriptor_type)
                    .build()
            );
        }

        binding_map.insert(set.0, descriptor_set_bindings);
    }

    Shader::new(shader, binding_map)
}

pub fn create_shader_module_from_bytes(device: Rc<RefCell<DeviceWrapper>>, name: &str, bytes: &[u8]) -> Shader
{
    let (reflection_module, shader) = {
        let reflection_module = rspirv_reflect::Reflection::new_from_spirv(bytes)
            .expect(&format!("Failed to parse shader for reflection data for {}", name));

        let create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::ShaderModuleCreateFlags::empty(),
            code_size: bytes.len(),
            p_code: bytes.as_ptr() as *const u32
        };

        let shader = DeviceWrapper::create_shader(device, name, &create_info);

        (reflection_module, shader)
    };

    // TODO: Add support for compute descriptor set bindings (could just use VK_SHADER_STAGE_ALL)
    // TODO: Add support for immutable samplers
    let mut binding_map : HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>> = HashMap::new();
    let descriptor_sets_reflection = reflection_module.get_descriptor_sets()
        .expect("Failed to get descriptor sets for reflected shader");
    for set in descriptor_sets_reflection {
        let mut descriptor_set_bindings: Vec<vk::DescriptorSetLayoutBinding> = Vec::new();
        for binding_reflect in set.1 {
            let binding_count = match binding_reflect.1.binding_count {
                BindingCount::One => { 1 }
                BindingCount::StaticSized(size) => {size}
                BindingCount::Unbounded => {
                    panic!("Unbounded descriptor binding count not supported");
                }
            };

            let descriptor_type = vk::DescriptorType::from_raw(binding_reflect.1.ty.0 as i32);

            descriptor_set_bindings.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding_reflect.0)
                    .stage_flags(vk::ShaderStageFlags::empty())
                    .descriptor_count(binding_count as u32)
                    .descriptor_type(descriptor_type)
                    .build()
            );
        }

        binding_map.insert(set.0, descriptor_set_bindings);
    }

    Shader::new(shader, binding_map)
}

#[derive(Clone)]
pub struct Shader
{
    pub shader: DeviceShader,
    pub descriptor_bindings: HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>>
}

impl Shader
{
    pub fn new(
        shader: DeviceShader,
        descriptor_bindings: HashMap<u32, Vec<vk::DescriptorSetLayoutBinding>>) -> Shader
    {
        Shader {
            shader,
            descriptor_bindings
        }
    }
}

pub struct ShaderManager
{
    shader_cache: HashMap<String, Rc<RefCell<Shader>>>
}

impl Debug for ShaderManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShaderManager")
            .field("cached shaders", &self.shader_cache.keys().len())
            .finish()
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

    pub fn load_shader(&mut self, device: Rc<RefCell<DeviceWrapper>>, file_name: &str) -> Rc<RefCell<Shader>>
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
                let sm = Rc::new(RefCell::new(create_shader_module(device, &full_name)));
                self.shader_cache.insert(full_name, sm.clone());
                sm
            }
        }
    }
}