use core::ffi::c_void;
use alloc::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use ash::vk;
use gpu_allocator::MemoryLocation;
use imgui::Ui;
use api_types::buffer::BufferCreateInfo;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::resource::DeviceResource;
use api_types::device::interface::DeviceInterface;
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use profiling::{enter_gpu_span, enter_span};
use crate::example::Example;

pub struct UBO {
    pub color: [f32; 3]
}
pub struct UboExample {
    uniform_buffer: Arc<Mutex<DeviceResource>>,
    vert_shader: Arc<Mutex<shader::Shader>>,
    frag_shader: Arc<Mutex<shader::Shader>>
}

impl Example for UboExample {
    fn get_name(&self) -> &'static str {
        "UBO"
    }

    fn execute(
        &self,
        _device: DeviceInterface,
        _allocator: Arc<Mutex<ResourceAllocator>>,
        _imgui_ui: &mut Ui,
        back_buffer: AttachmentReference) -> Vec<PassType> {

        let vertex_state_create = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_attribute_descriptions(&[])
            .vertex_binding_descriptions(&[]);

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let pipeline_description = Arc::new(PipelineDescription::new(
            Default::default(),
            Default::default(),
            dynamic_states,
            RasterizationType::Standard,
            DepthStencilType::Disable,
            BlendType::None,
            "ubo",
            self.vert_shader.clone(),
            self.frag_shader.clone()));
        
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
                move |device: DeviceInterface,
                     command_buffer: vk::CommandBuffer | {

                    enter_span!(tracing::Level::TRACE, "Draw Triangle");
                    enter_gpu_span!("Draw Triangle GPU", "examples", &device.get(), &command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);

                    let viewport = vk::Viewport::default()
                        .x(0.0)
                        .y(0.0)
                        .width(800.0)
                        .height(600.0)
                        .min_depth(0.0)
                        .max_depth(1.0);

                    let scissor = vk::Rect2D::default()
                        .offset(vk::Offset2D{x: 0, y: 0})
                        .extent(vk::Extent2D::default().width(800).height(600));

                    unsafe {
                        device.get().cmd_set_viewport(
                            command_buffer,
                            0,
                            std::slice::from_ref(&viewport));

                        device.get().cmd_set_scissor(
                            command_buffer,
                            0,
                            std::slice::from_ref(&scissor));

                        device.get().cmd_draw(
                            command_buffer,
                            3,
                            1,
                            0,
                            0);
                    }
                }
            ))
            .build()
            .expect("Failed to create UBO passnode");

        vec![PassType::Graphics(passnode)]
    }
}

impl UboExample {
    pub fn new(
        device: DeviceInterface,
        allocator: Arc<Mutex<ResourceAllocator>>) -> Self{
        let ubo_create = BufferCreateInfo::new(
            vk::BufferCreateInfo::default()
                .size(std::mem::size_of::<UBO>() as vk::DeviceSize)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            "ubo_example_buffer".to_string()
        );

        let ubo = device.create_buffer(
            0, // TODO: create buffer handle
            &ubo_create,
            allocator.clone(),
            MemoryLocation::CpuToGpu);

        let ubo_value = UBO {
            color: [1.0, 0.0, 0.0]
        };

        device.update_buffer(&ubo, |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &ubo_value,
                    mapped_memory as *mut UBO,
                    1);
            }
        });

        let vert_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(device.clone(), "ubo-vert", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ubo-vert.spv")))));
        let frag_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(device.clone(), "ubo-frag", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/ubo-frag.spv")))));

        UboExample {
            uniform_buffer: Arc::new(Mutex::new(ubo)),
            vert_shader,
            frag_shader
        }
    }
}