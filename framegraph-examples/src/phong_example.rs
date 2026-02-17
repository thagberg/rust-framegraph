use core::ffi::c_void;
use std::sync::{Arc, Mutex};
use ash::vk;
use gpu_allocator::MemoryLocation;
use imgui::Ui;
use api_types::buffer::BufferCreateInfo;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::resource::DeviceResource;
use api_types::device::interface::DeviceInterface;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader::{self, Shader};
use profiling::{enter_gpu_span, enter_span};
use crate::example::Example;
extern crate nalgebra_glm as glm;

#[repr(C)]
struct PhongUniforms {
    model: glm::Mat4,
    view: glm::Mat4,
    proj: glm::Mat4,
    light_pos: glm::Vec4,
    view_pos: glm::Vec4,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

pub struct PhongExample {
    uniform_buffer: Arc<Mutex<DeviceResource>>,
    vertex_buffer: Arc<Mutex<DeviceResource>>,
    vertex_shader: Arc<Mutex<Shader>>,
    fragment_shader: Arc<Mutex<Shader>>,
    rotation: f32,
}

impl Example for PhongExample {
    fn get_name(&self) -> &'static str {
        "Phong"
    }

    fn execute(
        &self,
        device: DeviceInterface,
        _allocator: Arc<Mutex<ResourceAllocator>>,
        imgui_ui: &mut Ui,
        back_buffer: AttachmentReference) -> Vec<PassType> {

        // Update uniforms
        let rotation = self.rotation;
        device.update_buffer(&self.uniform_buffer.lock().unwrap(), |mapped_memory: *mut c_void, _size: u64| {
            let model = glm::rotate(&glm::Mat4::identity(), rotation, &glm::vec3(0.0, 1.0, 0.0));
            let view = glm::look_at(
                &glm::vec3(0.0, 0.0, 3.0),
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 1.0, 0.0)
            );
            let proj = glm::perspective(800.0 / 600.0, 45.0f32.to_radians(), 0.1, 100.0);
            
            let uniforms = PhongUniforms {
                model,
                view,
                proj,
                light_pos: glm::vec4(2.0, 2.0, 2.0, 1.0),
                view_pos: glm::vec4(0.0, 0.0, 3.0, 1.0),
            };

            unsafe {
                core::ptr::copy_nonoverlapping(
                    &uniforms,
                    mapped_memory as *mut PhongUniforms,
                    1);
            }
        });

        imgui_ui.text(format!("Phong Shading Example"));
        imgui_ui.text(format!("Rotation: {:.2}", rotation));

        let vertex_binding = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX);

        let vertex_attributes = vec![
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(12),
        ];

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let pipeline_description = Arc::new(PipelineDescription::new(
            vec![vertex_binding],
            vertex_attributes,
            dynamic_states,
            RasterizationType::CullBack,
            DepthStencilType::Disable,
            BlendType::None,
            "phong",
            self.vertex_shader.clone(),
            self.fragment_shader.clone()));

        let ubo_binding = ResourceBinding {
            resource: self.uniform_buffer.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo {
                    offset: 0,
                    range: std::mem::size_of::<PhongUniforms>() as vk::DeviceSize
                }),
                set: 0,
                slot: 0,
                stage: vk::PipelineStageFlags::ALL_GRAPHICS,
                access: vk::AccessFlags::SHADER_READ
            },
        };

        let vertex_buf = self.vertex_buffer.clone();

        let passnode = GraphicsPassNode::builder("phong_Pass".to_string())
            .pipeline_description(pipeline_description)
            .read(ubo_binding)
            .render_target(back_buffer)
            .fill_commands(Box::new(
                move |device: DeviceInterface,
                      command_buffer: vk::CommandBuffer| {

                    enter_span!(tracing::Level::TRACE, "Draw Phong Cube");
                    enter_gpu_span!("Draw Phong Cube GPU", "examples", &device.get(), &command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);

                    let viewport = vk::Viewport::default()
                        .x(0.0)
                        .y(0.0)
                        .width(800.0)
                        .height(600.0)
                        .min_depth(0.0)
                        .max_depth(1.0);

                    let scissor = vk::Rect2D::default()
                        .offset(vk::Offset2D { x: 0, y: 0 })
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

                        let vertex_buffer = vertex_buf.lock().unwrap();
                        device.get().cmd_bind_vertex_buffers(
                            command_buffer,
                            0,
                            std::slice::from_ref(&vertex_buffer.get_buffer().get()),
                            &[0]);

                        device.get().cmd_draw(
                            command_buffer,
                            36,
                            1,
                            0,
                            0);
                    }
                }
            ))
            .build()
            .expect("Failed to create Phong passnode");

        vec![PassType::Graphics(passnode)]
    }

    fn update(&mut self, delta_time: f32) {
        self.rotation += delta_time;
    }
}

impl PhongExample {
    pub fn new(
        device: DeviceInterface,
        _render_context: &VulkanRenderContext,
        allocator: Arc<Mutex<ResourceAllocator>>) -> Self {

        // Create cube vertices with normals
        let vertices: Vec<Vertex> = vec![
            // Front face
            Vertex { pos: [-0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [-0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [-0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
            
            // Back face
            Vertex { pos: [0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [-0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [-0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
            
            // Top face
            Vertex { pos: [-0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [-0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [-0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
            
            // Bottom face
            Vertex { pos: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [-0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
            
            // Right face
            Vertex { pos: [0.5, -0.5, 0.5], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [0.5, -0.5, -0.5], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [0.5, 0.5, -0.5], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [0.5, 0.5, -0.5], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [0.5, 0.5, 0.5], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [0.5, -0.5, 0.5], normal: [1.0, 0.0, 0.0] },
            
            // Left face
            Vertex { pos: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5, -0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5, 0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5, 0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5, 0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
        ];

        let vertex_buffer_size = (std::mem::size_of::<Vertex>() * vertices.len()) as vk::DeviceSize;

        let vertex_buffer_create = BufferCreateInfo::new(
            vk::BufferCreateInfo::default()
                .size(vertex_buffer_size)
                .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            "phong_vertex_buffer".to_string()
        );

        let vertex_buffer = device.create_buffer(
            0,
            &vertex_buffer_create,
            allocator.clone(),
            MemoryLocation::CpuToGpu);

        device.update_buffer(&vertex_buffer, |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    vertices.as_ptr(),
                    mapped_memory as *mut Vertex,
                    vertices.len());
            }
        });

        let ubo_create = BufferCreateInfo::new(
            vk::BufferCreateInfo::default()
                .size(std::mem::size_of::<PhongUniforms>() as vk::DeviceSize)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            "phong_uniform_buffer".to_string()
        );

        let uniform_buffer = device.create_buffer(
            0,
            &ubo_create,
            allocator.clone(),
            MemoryLocation::CpuToGpu);

        let vertex_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "phong-vert",
                include_bytes!(concat!(env!("SHADER_DIR"), "/phong-vert.spv")))));
        
        let fragment_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "phong-frag",
                include_bytes!(concat!(env!("SHADER_DIR"), "/phong-frag.spv")))));

        PhongExample {
            uniform_buffer: Arc::new(Mutex::new(uniform_buffer)),
            vertex_buffer: Arc::new(Mutex::new(vertex_buffer)),
            vertex_shader,
            fragment_shader,
            rotation: 0.0,
        }
    }
}