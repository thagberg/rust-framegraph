use core::ffi::c_void;
use alloc::rc::Rc;
use std::cell::RefCell;
use ash::vk;
use gpu_allocator::MemoryLocation;
use imgui::Ui;
use context::api_types::buffer::BufferCreateInfo;
use context::api_types::device::{DeviceResource, DeviceWrapper};
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use crate::example::Example;

pub struct UBO {
    pub color: [f32; 3]
}
pub struct UboExample {
    active: bool,
    uniform_buffer: Rc<RefCell<DeviceResource>>,
    vert_shader: Rc<RefCell<shader::Shader>>,
    frag_shader: Rc<RefCell<shader::Shader>>
}

impl Example for UboExample {
    fn get_name(&self) -> &'static str {
        "UBO"
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn get_active(&self) -> bool {
        self.active
    }

    fn execute(&self, imgui_ui: &Ui, back_buffer: AttachmentReference) -> Vec<PassType> {
        let vertex_state_create = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&[])
            .vertex_binding_descriptions(&[]);

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let pipeline_description = PipelineDescription::new(
            Default::default(),
            dynamic_states,
            RasterizationType::Standard,
            DepthStencilType::Disable,
            BlendType::None,
            "ubo",
            self.vert_shader.clone(),
            self.frag_shader.clone());
        
        let ubo_binding = ResourceBinding {
            resource: self.uniform_buffer.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo{
                    offset: 0,
                    range: std::mem::size_of::<UBO>() as vk::DeviceSize }),
                set: 0,
                slot: 0,
                stage: vk::PipelineStageFlags::ALL_GRAPHICS,
                access: vk::AccessFlags::SHADER_READ
            },
        };

        let passnode = GraphicsPassNode::builder("ubo_Pass".to_string())
            .pipeline_description(pipeline_description)
            .read(ubo_binding)
            .render_target(back_buffer)
            .fill_commands(Box::new(
                move |render_ctx: &VulkanRenderContext,
                     command_buffer: &vk::CommandBuffer | {

                }
            ))
            .build()
            .expect("Failed to create UBO passnode");

        vec![PassType::Graphics(passnode)]
    }
}

impl UboExample {
    pub fn new(device: Rc<RefCell<DeviceWrapper>>) -> Self{
        let ubo_create = BufferCreateInfo::new(
            vk::BufferCreateInfo::builder()
                .size(std::mem::size_of::<UBO>() as vk::DeviceSize)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .build(),
            "ubo_example_buffer".to_string()
        );

        let ubo = DeviceWrapper::create_buffer(
            device.clone(),
            &ubo_create,
            MemoryLocation::CpuToGpu);

        let ubo_value = UBO {
            color: [1.0, 0.0, 0.0]
        };

        device.borrow().update_buffer(&ubo, |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &ubo_value,
                    mapped_memory as *mut UBO,
                    1);
            }
        });

        let vert_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(device.clone(), "ubo-vert", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ubo-vert.spv")))));
        let frag_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(device.clone(), "ubo-frag", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ubo-frag.spv")))));

        UboExample {
            active: false,
            uniform_buffer: Rc::new(RefCell::new(ubo)),
            vert_shader,
            frag_shader
        }
    }
}