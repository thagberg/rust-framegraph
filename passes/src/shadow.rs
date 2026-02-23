use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::interface::DeviceInterface;
use api_types::device::resource::DeviceResource;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use framegraph::shader::Shader;
use profiling::{enter_gpu_span, enter_span};

pub struct ShadowMappingObjects {
    pub vertex_buffer: Arc<Mutex<DeviceResource>>,
    pub index_buffer: Arc<Mutex<DeviceResource>>,
    pub index_count: u32,
    pub vertex_binding: vk::VertexInputBindingDescription,
    pub vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    pub light_mvp_buffer: Arc<Mutex<DeviceResource>>,
}

pub struct ShadowPass {
    vertex_shader: Arc<Mutex<Shader>>,
    fragment_shader: Arc<Mutex<Shader>>,
}

impl ShadowPass {
    pub fn new(device: DeviceInterface) -> Self {
        let vertex_shader = Arc::new(Mutex::new(shader::create_shader_module_from_bytes(
            device.clone(),
            "shadow-vert",
            include_bytes!(concat!(env!("SHADER_DIR"), "/shadow-vert.spv")),
        )));
        let fragment_shader = Arc::new(Mutex::new(shader::create_shader_module_from_bytes(
            device.clone(),
            "shadow-frag",
            include_bytes!(concat!(env!("SHADER_DIR"), "/shadow-frag.spv")),
        )));

        Self {
            vertex_shader,
            fragment_shader,
        }
    }

    pub fn generate_pass(
        &self,
        depth_target: AttachmentReference,
        objects: Vec<ShadowMappingObjects>,
    ) -> PassType {
        // Assuming all objects can use the same pipeline for shadow mapping if their vertex layout is same.
        // For simplicity, we'll use the first object's vertex layout for the pipeline.
        // In a real scenario, we might need multiple passes or multiple pipelines.
        let (vertex_binding, vertex_attributes) = if let Some(first) = objects.first() {
            (vec![first.vertex_binding], first.vertex_attributes.clone())
        } else {
            (vec![], vec![])
        };

        let pipeline_description = Arc::new(PipelineDescription::new(
            vertex_binding,
            vertex_attributes,
            vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR],
            RasterizationType::CullBack,
            DepthStencilType::Enable,
            BlendType::None,
            "shadow_map",
            self.vertex_shader.clone(),
            self.fragment_shader.clone(),
        ));

        let mut pass_node_builder = GraphicsPassNode::builder("shadow_pass".to_string())
            .pipeline_description(pipeline_description)
            .depth_target(depth_target.clone());

        for object in &objects {
            let mvp_binding = ResourceBinding {
                resource: object.light_mvp_buffer.clone(),
                binding_info: BindingInfo {
                    binding_type: BindingType::Buffer(BufferBindingInfo {
                        offset: 0,
                        range: vk::WHOLE_SIZE,
                    }),
                    set: 0,
                    slot: 0,
                    stage: vk::PipelineStageFlags::VERTEX_SHADER,
                    access: vk::AccessFlags::SHADER_READ,
                },
            };
            pass_node_builder = pass_node_builder.read(mvp_binding);
        }

        let pass_node = pass_node_builder
            .fill_commands(Box::new(
                move |device: DeviceInterface, command_buffer: vk::CommandBuffer| {
                    enter_span!(tracing::Level::TRACE, "shadow_pass");
                    enter_gpu_span!(
                        "shadow_pass_gpu",
                        "Passes",
                        device.get(),
                        &command_buffer,
                        vk::PipelineStageFlags::ALL_GRAPHICS
                    );

                    let (width, height) = {
                        let extent = depth_target.resource_image.lock().unwrap().get_image().extent;
                        (extent.width as f32, extent.height as f32)
                    };

                    let viewport = vk::Viewport::default()
                        .x(0.0)
                        .y(0.0)
                        .width(width)
                        .height(height)
                        .min_depth(0.0)
                        .max_depth(1.0);

                    let scissor = vk::Rect2D::default()
                        .offset(vk::Offset2D { x: 0, y: 0 })
                        .extent(vk::Extent2D {
                            width: width as u32,
                            height: height as u32,
                        });

                    unsafe {
                        device.get().cmd_set_viewport(command_buffer, 0, std::slice::from_ref(&viewport));
                        device.get().cmd_set_scissor(command_buffer, 0, std::slice::from_ref(&scissor));

                        for object in &objects {
                            let vb = object.vertex_buffer.lock().unwrap();
                            let ib = object.index_buffer.lock().unwrap();

                            device.get().cmd_bind_vertex_buffers(
                                command_buffer,
                                0,
                                &[vb.get_buffer().buffer],
                                &[0],
                            );
                            device.get().cmd_bind_index_buffer(
                                command_buffer,
                                ib.get_buffer().buffer,
                                0,
                                vk::IndexType::UINT32,
                            );

                            device.get().cmd_draw_indexed(
                                command_buffer,
                                object.index_count,
                                1,
                                0,
                                0,
                                0,
                            );
                        }
                    }
                },
            ))
            .build()
            .expect("Failed to build shadow pass node");

        PassType::Graphics(pass_node)
    }
}
