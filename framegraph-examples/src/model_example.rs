use alloc::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use ash::vk;
use ash::vk::BufferDeviceAddressCreateInfoEXT;
use imgui::Ui;
use gltf::{Gltf, Semantic};
use gltf::accessor::{DataType, Dimensions};
use gpu_allocator::MemoryLocation;
use image::error::UnsupportedErrorKind::Format;
use context::api_types::buffer::{BufferCreateInfo, BufferWrapper};
use context::api_types::device::{DeviceResource, DeviceWrapper, ResourceType};
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;
use once_cell::sync::Lazy;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::shader::Shader;
use util::camera::Camera;
use glm;
use context::render_context::RenderContext;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ResourceBinding};
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use crate::example::Example;

struct Vert {
    pub pos: glm::Vec3,
    pub normal: glm::Vec3,
    pub uv: glm::Vec2
}

pub struct GltfModel {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>
}

struct MVP {
    model: glm::TMat4<f32>,
    view: glm::TMat4<f32>,
    proj: glm::TMat4<f32>
}

static ATTRIBUTE_LOOKUP: Lazy<HashMap<gltf::mesh::Semantic, u32>> = Lazy::new(|| HashMap::from([
    (gltf::mesh::Semantic::Positions, 0),
    (gltf::mesh::Semantic::Normals, 1),
    (gltf::mesh::Semantic::TexCoords(0), 2)
]));

const VERTEX_BINDING:  vk::VertexInputBindingDescription = vk::VertexInputBindingDescription {
    binding: 0,
    stride: std::mem::size_of::<Vert>() as u32,
    input_rate: vk::VertexInputRate::VERTEX,
};

pub struct RenderMesh {
    // TODO: add primitive topology (also need to support this in pipeline.rs)
    vertex_buffer: Rc<RefCell<DeviceResource>>,
    index_buffer: Option<Rc<RefCell<DeviceResource>>>,
    num_indices: usize,
    vertex_binding: vk::VertexInputBindingDescription,
    vertex_attributes: Vec<vk::VertexInputAttributeDescription>
}

#[derive(Eq, PartialEq, Hash)]
pub struct FormatKey {
    data_type: u8,
    num_components: u8
}


// static mut FORMAT_LOOKUP: Lazy<HashMap<FormatKey, vk::Format>> = Lazy::new(|| HashMap::new());
static FORMAT_LOOKUP: Lazy<HashMap<FormatKey, vk::Format>> = Lazy::new(|| HashMap::from([
    // I8 formats
    (FormatKey { data_type: DataType::I8 as u8, num_components: 1, }, vk::Format::R8_SINT),
    (FormatKey { data_type: DataType::I8 as u8, num_components: 2, }, vk::Format::R8G8_SINT),
    (FormatKey { data_type: DataType::I8 as u8, num_components: 3, }, vk::Format::R8G8B8_SINT),
    (FormatKey { data_type: DataType::I8 as u8, num_components: 4, }, vk::Format::R8G8B8A8_SINT),

    // I16 formats
    (FormatKey { data_type: DataType::I16 as u8, num_components: 1, }, vk::Format::R16_SINT),
    (FormatKey { data_type: DataType::I16 as u8, num_components: 2, }, vk::Format::R16G16_SINT),
    (FormatKey { data_type: DataType::I16 as u8, num_components: 3, }, vk::Format::R16G16B16_SINT),
    (FormatKey { data_type: DataType::I16 as u8, num_components: 4, }, vk::Format::R16G16B16A16_SINT),

    // U8 formats
    (FormatKey { data_type: DataType::U8 as u8, num_components: 1, }, vk::Format::R8_UINT),
    (FormatKey { data_type: DataType::U8 as u8, num_components: 2, }, vk::Format::R8G8_UINT),
    (FormatKey { data_type: DataType::U8 as u8, num_components: 3, }, vk::Format::R8G8B8_UINT),
    (FormatKey { data_type: DataType::U8 as u8, num_components: 4, }, vk::Format::R8G8B8A8_UINT),

    // U16 formats
    (FormatKey { data_type: DataType::U16 as u8, num_components: 1, }, vk::Format::R16_UINT),
    (FormatKey { data_type: DataType::U16 as u8, num_components: 2, }, vk::Format::R16G16_UINT),
    (FormatKey { data_type: DataType::U16 as u8, num_components: 3, }, vk::Format::R16G16B16_UINT),
    (FormatKey { data_type: DataType::U16 as u8, num_components: 4, }, vk::Format::R16G16B16A16_UINT),

    // U32 formats
    (FormatKey { data_type: DataType::U32 as u8, num_components: 1, }, vk::Format::R32_UINT),
    (FormatKey { data_type: DataType::U32 as u8, num_components: 2, }, vk::Format::R32G32_UINT),
    (FormatKey { data_type: DataType::U32 as u8, num_components: 3, }, vk::Format::R32G32B32_UINT),
    (FormatKey { data_type: DataType::U32 as u8, num_components: 4, }, vk::Format::R32G32B32A32_UINT),

    // float formats
    (FormatKey { data_type: DataType::F32 as u8, num_components: 1, }, vk::Format::R32_SFLOAT),
    (FormatKey { data_type: DataType::F32 as u8, num_components: 2, }, vk::Format::R32G32_SFLOAT),
    (FormatKey { data_type: DataType::F32 as u8, num_components: 3, }, vk::Format::R32G32B32_SFLOAT),
    (FormatKey { data_type: DataType::F32 as u8, num_components: 4, }, vk::Format::R32G32B32A32_SFLOAT)
]));

pub fn get_vk_format(data_type: DataType, dimensions: Dimensions) -> vk::Format {
    let num_components = match dimensions { Dimensions::Scalar => 1,
        Dimensions::Vec2 => 2,
        Dimensions::Vec3 => 3,
        Dimensions::Vec4 => 4,
        _ => {
            // Currently do not support matrix attribute types
            panic!("Invalid format dimensions: {:?}", dimensions)
        }

    };

    let key = FormatKey {
        data_type: data_type as u8,
        num_components
    };

    let result = unsafe {
        *FORMAT_LOOKUP.get(&key).expect("Invalid format or num components")
    };
    result
}


pub struct ModelExample {
    vertex_shader: Rc<RefCell<Shader>>,
    fragment_shader: Rc<RefCell<Shader>>,
    camera: Camera,
    duck_model: GltfModel,
    render_meshes: Vec<RenderMesh>
}

impl Example for ModelExample {
    fn get_name(&self) -> &'static str {
        "Model Render"
    }

    fn execute(&self, device: Rc<RefCell<DeviceWrapper>>, imgui_ui: &mut Ui, back_buffer: AttachmentReference) -> Vec<PassType> {
        let mut passes: Vec<PassType> = Vec::new();

        // create UBO for MVP
        let mvp_buffer = {
            let create_info = BufferCreateInfo::new(
                vk::BufferCreateInfo::builder()
                    .size(std::mem::size_of::<MVP>() as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                    .build(),
                "MVP_buffer".to_string()
            );
            let buffer = DeviceWrapper::create_buffer(
                device.clone(),
                &create_info,
                MemoryLocation::CpuToGpu
            );

            device.borrow().update_buffer(&buffer, |mapped_memory: *mut c_void, _size: u64| {
                let mvp = MVP {
                    model: glm::identity(),
                    view: self.camera.view.clone(),
                    proj: self.camera.projection.clone(),
                };

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        &mvp,
                        mapped_memory as *mut MVP,
                        1
                    );
                }
            });

            Rc::new(RefCell::new(buffer))
        };

        let mvp_binding = ResourceBinding {
            resource: mvp_buffer.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo{
                    offset: 0,
                    range: std::mem::size_of::<MVP>() as vk::DeviceSize,
                }),
                set: 0,
                slot: 0,
                stage: vk::PipelineStageFlags::VERTEX_SHADER,
                access: vk::AccessFlags::SHADER_READ,
            }
        };

        for render_mesh in &self.render_meshes {
            let dynamic_states = vec!(vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR);
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_binding_descriptions(std::slice::from_ref(&render_mesh.vertex_binding))
                .vertex_attribute_descriptions(&render_mesh.vertex_attributes)
                .build();

            let pipeline_description = PipelineDescription::new(
                vertex_input,
                dynamic_states,
                RasterizationType::Standard,
                DepthStencilType::Disable,
                BlendType::None,
                "gltf-model-draw",
                self.vertex_shader.clone(),
                self.fragment_shader.clone());

            let (viewport, scissor) = {
                let extent = back_buffer.resource_image.borrow().get_image().extent;
                let v = vk::Viewport::builder()
                    .x(0.0)
                    .y(0.0)
                    .width(extent.width as f32)
                    .height(extent.height as f32)
                    .min_depth(0.0)
                    .max_depth(1.0)
                    .build();

                let s = vk::Rect2D::builder()
                    .offset(vk::Offset2D{x: 0, y: 0})
                    .extent(vk::Extent2D{width: extent.width, height: extent.height})
                    .build();

                (v, s)
            };

            if let Some(ibo_ref) = &render_mesh.index_buffer {
                let ibo = ibo_ref.clone();
                let vbo = render_mesh.vertex_buffer.clone();
                let idx_length = render_mesh.num_indices;
                let passnode = GraphicsPassNode::builder("model_render".to_string())
                    .pipeline_description(pipeline_description)
                    .render_target(back_buffer.clone())
                    .read(mvp_binding.clone())
                    .tag(render_mesh.vertex_buffer.clone())
                    .tag(ibo.clone())
                    .viewport(viewport)
                    .scissor(scissor)
                    .fill_commands(Box::new(
                        move | render_ctx: &VulkanRenderContext,
                               command_buffer: &vk::CommandBuffer | {
                            println!("Rendering glTF model");

                            unsafe {
                                // set vertex buffer
                                {
                                    if let ResourceType::Buffer(vb) = vbo.borrow().resource_type.as_ref().unwrap() {
                                        render_ctx.get_device().borrow().get().cmd_bind_vertex_buffers(
                                            *command_buffer,
                                            0,
                                            &[vb.buffer],
                                            &[0 as vk::DeviceSize]
                                        );
                                    } else {
                                        panic!("Invalid vertex buffer for gltf draw");
                                    }
                                }

                                // set index buffer
                                {
                                    if let ResourceType::Buffer(ib) = ibo.borrow().resource_type.as_ref().unwrap() {
                                        render_ctx.get_device().borrow().get().cmd_bind_index_buffer(
                                            *command_buffer,
                                            ib.buffer,
                                            0 as vk::DeviceSize,
                                            vk::IndexType::UINT16
                                        );
                                    } else {
                                        panic!("Invalid index buffer for gltf draw");
                                    }
                                }

                                render_ctx.get_device().borrow().get().cmd_draw_indexed(
                                    *command_buffer,
                                    idx_length as u32,
                                    1,
                                    0,
                                    0,
                                    0
                                );
                            }
                        }
                    ))
                    .build()
                    .expect("Failed to create glTF Model pass");

                passes.push(PassType::Graphics(passnode));
            }
        }

        passes
    }
}

impl ModelExample {
    pub fn new(device: Rc<RefCell<DeviceWrapper>>) -> Self {
        let duck_import = gltf::import("assets/models/gltf/duck/Duck.gltf");
        let duck_gltf = match duck_import {
            Ok(gltf) => {
                GltfModel {
                    document: gltf.0,
                    buffers: gltf.1,
                    images: gltf.2
                }
            },
            Err(e) => {
                panic!("Failed to open Duck gltf model: {}", e)
            }
        };

        // create images and upload gltf images data

        // prepare meshes
        //  * vertex layout
        //  * buffer bindings
        //  * image bindings
        // each node could be a separate object in the scene
        let mut meshes: Vec<RenderMesh> = Vec::new();
        for node in duck_gltf.document.nodes() {
            if let Some(mesh) = node.mesh() {
                for (i, primitive) in mesh.primitives().enumerate() {
                    let mut ibo: Option<Rc<RefCell<DeviceResource>>> = None;

                    let primitive_name = {
                        if let Some(mesh_name) = mesh.name() {
                            format!("{}_{}", mesh_name, i)
                        } else {
                            format!("UnknownMesh_{}", i)
                        }
                    };

                    let mode = primitive.mode();
                    let mut num_indices = 0;
                    if let Some(indices_accessor) = primitive.indices() {
                        // * create GPU index buffer
                        num_indices = indices_accessor.count();
                        let index_size = indices_accessor.size();
                        let ibo_size = indices_accessor.count() * index_size;
                        indices_accessor.data_type();
                        ibo = Some({
                            let ibo_create = BufferCreateInfo::new(
                                vk::BufferCreateInfo::builder()
                                    .size(ibo_size as vk::DeviceSize)
                                    .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                    .build(),
                                primitive_name.clone()
                            );
                            Rc::new(RefCell::new(DeviceWrapper::create_buffer(
                                device.clone(),
                                &ibo_create,
                                MemoryLocation::CpuToGpu
                            )))
                        });

                        // * memory map the buffer
                        // * use the indices accessor to copy indices data into the GPU buffer
                        device.borrow().update_buffer(&ibo.as_ref().unwrap().borrow(), |mapped_memory: *mut c_void, _size: u64| {
                            unsafe {
                                let view = indices_accessor.view().expect("Failed to get view for index buffer");
                                let buffer_data = duck_gltf.buffers.get(view.buffer().index())
                                    .expect("Failed to get buffer data for index buffer");
                                let source_offset = view.offset() + indices_accessor.offset();
                                core::ptr::copy_nonoverlapping(
                                    buffer_data.0.as_ptr().byte_add(source_offset),
                                    mapped_memory as *mut u8,
                                    ibo_size);
                            }
                        });
                    }

                    let mut vertex_data_size = 0usize;
                    let mut vertex_size = 0usize;
                    let mut vertex_attributes: Vec<vk::VertexInputAttributeDescription> = Vec::new();
                    // need to do an initial pass over attributes to calculate total VBO size and vertex size
                    for (semantic, attribute_accessor) in primitive.attributes() {
                        // only keep attributes which are used in the renderer
                        if let Some(found_location) = ATTRIBUTE_LOOKUP.get(&semantic) {
                            vertex_data_size += attribute_accessor.count() * attribute_accessor.size();
                            let offset = vertex_size; // TODO: probably need to deal with alignment here
                            vertex_size += attribute_accessor.size();
                            // attribute_accessor.data_type()

                            // create a vertex input attribute per primitive attribute
                            let format = get_vk_format(attribute_accessor.data_type(), attribute_accessor.dimensions());
                            vertex_attributes.push(
                                vk::VertexInputAttributeDescription::builder()
                                    .format(format)
                                    .binding(0) // TODO: assuming a single vertex buffer currently
                                    .location(*found_location)
                                    .offset(offset as u32)
                                    .build()
                            );
                        }
                    }

                    // create vertex buffer
                    let vbo = {
                        let vbo_create = BufferCreateInfo::new(
                            vk::BufferCreateInfo::builder()
                                .size(vertex_data_size as vk::DeviceSize)
                                .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                .build(),
                            primitive_name.clone()
                        );
                        DeviceWrapper::create_buffer(
                            device.clone(),
                            &vbo_create,
                            MemoryLocation::CpuToGpu
                        )
                    };

                    // iterate over attributes again and copy them from mesh buffers into the VBO
                    device.borrow().update_buffer(&vbo, |mapped_memory: *mut c_void, _size: u64| {
                        unsafe {
                            let mut vertex_offset = 0;
                            for (semantic, attribute_accessor) in primitive.attributes() {
                                let view = attribute_accessor.view().expect("Failed to get view for vertex attribute");
                                let buffer_data = duck_gltf.buffers.get(view.buffer().index())
                                    .expect("Failed to get buffer for vertex attribute");
                                let stride = match view.stride() {
                                    None => {1} // I think this is a safe assumption?
                                    Some(s) => {s}
                                };

                                // source_offset is the offset into the source buffer defined by the buffer view (base) and the accessor
                                let mut source_offset = view.offset() + attribute_accessor.offset();
                                // dest_offset is the offset into the dest buffer defined by the index of the element being written and the
                                //  per-vertex offset of the current attribute
                                let mut dest_offset = vertex_offset;

                                let num_elements = attribute_accessor.count();
                                for i in (0..num_elements) {
                                    // for each element, copy the value from the source buffer into the dest buffer
                                    // for each element, source_offset will increment by the buffer view's stride
                                    // dest_offset will increment by the vertex size (attributes are interleaved in the dest buffer)

                                    core::ptr::copy_nonoverlapping(
                                        buffer_data.0.as_ptr().byte_add(source_offset),
                                        mapped_memory.byte_add(dest_offset) as *mut u8,
                                        attribute_accessor.size());

                                    source_offset += stride;
                                    dest_offset += vertex_size;
                                }

                                vertex_offset += attribute_accessor.size();
                            }
                        }
                    });

                    let render_mesh = RenderMesh {
                        vertex_buffer: Rc::new(RefCell::new(vbo)),
                        index_buffer: ibo,
                        num_indices,
                        vertex_binding: VERTEX_BINDING,
                        vertex_attributes
                    };
                    meshes.push(render_mesh);
                }
            }
        }

        let camera = Camera::new(
            1.5,
            0.66,
            1.0,
            10000.0,
            &glm::Vec3::new(0.0, 0.0, 0.0),
            &glm::Vec3::new(0.0, 0.0, 1.0),
            &glm::Vec3::new(0.0, 1.0, 0.0)
        );

        let vert_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "model-vert",
                include_bytes!(concat!(env!("OUT_DIR"), "/shaders/model-vert.spv")))));
        let frag_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "model-frag",
                include_bytes!(concat!(env!("OUT_DIR"), "/shaders/model-frag.spv")))));

        ModelExample{
            vertex_shader: vert_shader,
            fragment_shader: frag_shader,
            camera,
            duck_model: duck_gltf,
            render_meshes: meshes
        }
    }
}