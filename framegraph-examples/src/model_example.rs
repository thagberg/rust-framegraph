use alloc::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ops::Mul;
use std::sync::{Arc, Mutex};
use ash::vk;
use ash::vk::{Handle};
use imgui::{Condition, Ui};
use gltf::{Semantic};
use gltf::accessor::{DataType, Dimensions};
use gpu_allocator::MemoryLocation;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;
use once_cell::sync::Lazy;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::shader::Shader;
use util::camera::Camera;
use util::math::DecomposedMatrix;
use glm;
use glm::Vec4;
use gltf::camera::Projection;
use gltf::image::Source;
use gltf::json::accessor::{Type};
use api_types::buffer::BufferCreateInfo;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::resource::{DeviceResource, ResourceType};
use api_types::device::interface::DeviceInterface;
use api_types::image::{ImageCreateInfo, ImageType};
use context::render_context::RenderContext;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ImageBindingInfo, ResourceBinding};
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use profiling::{enter_gpu_span, enter_span};
use passes::clear;
use crate::example::Example;

#[derive(Default)]
#[repr(C)]
struct Vert {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2]
}

struct VertexAttributeAccessor<'a> {
    view: Option<gltf::buffer::View<'a>>,
    offset: usize,
    count: usize,
    size: usize
}

struct VertexAttribute<'a> {
    location: u32,
    format: vk::Format,
    accessor: Option<gltf::Accessor<'a>>
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
    vertex_buffer: Arc<Mutex<DeviceResource>>,
    index_buffer: Option<Arc<Mutex<DeviceResource>>>,
    num_indices: usize,
    vertex_binding: vk::VertexInputBindingDescription,
    vertex_attributes: [vk::VertexInputAttributeDescription; 3],
    transform: glm::TMat4<f32>,
    albedo_tex: Option<Arc<Mutex<DeviceResource>>>
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

fn get_vk_format(data_type: DataType, dimensions: Dimensions) -> vk::Format {
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

pub enum GlmType {
    Scalar(f32),
    Vec2(glm::TVec2<f32>),
    Vec3(glm::TVec3<f32>),
    Vec4(glm::TVec4<f32>)
}

fn get_size_per_component(data_type: DataType) -> usize {
    match data_type {
        DataType::I8 => 1,
        DataType::U8 => 1,
        DataType::I16 => 2,
        DataType::U16 => 2,
        DataType::U32 => 4,
        DataType::F32 => 4
    }
}

fn get_num_components_for_dimension(dimensions: Dimensions) -> usize {
    match dimensions {
        Type::Scalar => 1,
        Type::Vec2 => 2,
        Type::Vec3 => 3,
        Type::Vec4 => 4,
        _ => panic!("Only scalar and vector types supported")
    }
}

fn buffer_bytes_to_f32(data_pointer: *const u8, num_bytes: usize) -> f32 {
    // Per the glTF 2.0 spec, buffer data must be in little-endian form
    // https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#buffers-and-buffer-views-overview
    unsafe {
        let byte_array = {
            match num_bytes {
                1 => {
                    [0x00, 0x00, 0x00, data_pointer.read()]
                }
                2 => {
                    [0x00, 0x00, data_pointer.read(), data_pointer.byte_add(1).read()]
                }
                3 => {
                    [0x00, data_pointer.read(), data_pointer.byte_add(1).read(), data_pointer.byte_add(2).read()]
                }
                4 => {
                    [data_pointer.read(), data_pointer.byte_add(1).read(), data_pointer.byte_add(2).read(), data_pointer.byte_add(3).read()]
                },
                _ => {
                    panic!("Unsupported number of bytes to read into f32: {}", num_bytes)
                }
            }
        };

        f32::from_le_bytes(byte_array)
    }
}

unsafe fn get_vec2_from_gltf_buffer(data_type: DataType, dimensions: Dimensions, data_pointer: *const u8) -> glm::Vec2 {
    let bytes_per_component = get_size_per_component(data_type);
    let num_components = get_num_components_for_dimension(dimensions);
    assert_eq!(num_components, 2, "Can't read a vec2 from {} components", num_components);

    glm::Vec2::new(
        buffer_bytes_to_f32(data_pointer, bytes_per_component),
        buffer_bytes_to_f32(data_pointer.byte_add(bytes_per_component), bytes_per_component)
    )
}

unsafe fn get_vec3_from_gltf_buffer(data_type: DataType, dimensions: Dimensions, data_pointer: *const u8) -> glm::Vec3 {
    let bytes_per_component = get_size_per_component(data_type);
    let num_components = get_num_components_for_dimension(dimensions);
    assert_eq!(num_components, 3, "Can't read a vec3 from {} components", num_components);

    glm::Vec3::new(
        buffer_bytes_to_f32(data_pointer, bytes_per_component),
        buffer_bytes_to_f32(data_pointer.byte_add(bytes_per_component), bytes_per_component),
        buffer_bytes_to_f32(data_pointer.byte_add(2 * bytes_per_component), bytes_per_component),
    )
}

unsafe fn get_vec4_from_gltf_buffer(data_type: DataType, dimensions: Dimensions, data_pointer: *const u8) -> glm::Vec4 {
    let bytes_per_component = get_size_per_component(data_type);
    let num_components = get_num_components_for_dimension(dimensions);
    assert_eq!(num_components, 4, "Can't read a vec4 from {} components", num_components);

    glm::Vec4::new(
        buffer_bytes_to_f32(data_pointer, bytes_per_component),
        buffer_bytes_to_f32(data_pointer.byte_add(bytes_per_component), bytes_per_component),
        buffer_bytes_to_f32(data_pointer.byte_add(2 * bytes_per_component), bytes_per_component),
        buffer_bytes_to_f32(data_pointer.byte_add(3 * bytes_per_component), bytes_per_component),
    )
}

unsafe fn get_scalar_from_gltf_buffer(data_type: DataType, dimensions: Dimensions, data_pointer: *const u8) -> f32 {
    let bytes_per_component = get_size_per_component(data_type);
    let num_components = get_num_components_for_dimension(dimensions);
    assert_eq!(num_components, 1, "Can't read a scalar from {} components", num_components);

    buffer_bytes_to_f32(data_pointer, bytes_per_component)
}

fn get_glm_format(data_type: DataType, dimensions: Dimensions, data_pointer: *const u8) -> GlmType {
    let bytes_per_component = get_size_per_component(data_type);
    let num_components = get_num_components_for_dimension(dimensions);

    unsafe {
        match num_components {
            1 => {
                GlmType::Scalar(buffer_bytes_to_f32(data_pointer, bytes_per_component))
            },
            2 => {
                GlmType::Vec2(
                    get_vec2_from_gltf_buffer(data_type, dimensions, data_pointer)
                )
            },
            3 => {
                GlmType::Vec3(glm::Vec3::new(
                    buffer_bytes_to_f32(data_pointer, bytes_per_component),
                    buffer_bytes_to_f32(data_pointer.byte_add(bytes_per_component), bytes_per_component),
                    buffer_bytes_to_f32(data_pointer.byte_add(2 * bytes_per_component), bytes_per_component),
                ))
            },
            4 => {
                GlmType::Vec4(glm::Vec4::new(
                    buffer_bytes_to_f32(data_pointer, bytes_per_component),
                    buffer_bytes_to_f32(data_pointer.byte_add(bytes_per_component), bytes_per_component),
                    buffer_bytes_to_f32(data_pointer.byte_add(2 * bytes_per_component), bytes_per_component),
                    buffer_bytes_to_f32(data_pointer.byte_add(3 * bytes_per_component), bytes_per_component),
                ))
            },
            _ => {
                panic!("Only scalar and vector types supported")
            }
        }
    }
}

pub struct ModelExample {
    vertex_shader: Arc<Mutex<Shader>>,
    fragment_shader: Arc<Mutex<Shader>>,
    camera: Camera,
    duck_model: GltfModel,
    render_meshes: Vec<RenderMesh>
}

impl Example for ModelExample {
    fn get_name(&self) -> &'static str {
        "Model Render"
    }

    fn execute(
        &self,
        device: DeviceInterface,
        allocator: Arc<Mutex<ResourceAllocator>>,
        imgui_ui: &mut Ui,
        back_buffer: AttachmentReference) -> Vec<PassType> {

        enter_span!(tracing::Level::TRACE, "Generating Model Pass");

        // build UI
        imgui_ui.window("glTF Model")
            .size([300.0, 300.0], Condition::Once)
            .build(|| {

            });

        let mut passes: Vec<PassType> = Vec::new();

        let depth_attachment = {
            let depth_image = {
                let rt_extent = back_buffer.resource_image.lock().unwrap().get_image().extent.clone();
                let depth_create = vk::ImageCreateInfo::default()
                    .format(vk::Format::D32_SFLOAT)
                    .image_type(vk::ImageType::TYPE_2D)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    // .initial_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    // transfer_dst required for this to be clearable via vkCmdClearDepthStencilImage
                    // https://vulkan.lunarg.com/doc/view/1.3.290.0/windows/1.3-extensions/vkspec.html#VUID-vkCmdClearDepthStencilImage-pRanges-02660
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
                    .extent(rt_extent)
                    .mip_levels(1)
                    .array_layers(1);

                let image_create = ImageCreateInfo::new(
                    depth_create,
                    "model_example_depth".to_string(),
                    ImageType::Depth
                );

                device.create_image(
                    0, // TODO: create image handle
                    &image_create,
                    allocator.clone(),
                    MemoryLocation::GpuOnly
                )
            };

            AttachmentReference::new(
                Arc::new(Mutex::new(depth_image)),
                vk::SampleCountFlags::TYPE_1
            )
        };

        // add depth clear pass
        passes.push(clear::clear(
            depth_attachment.resource_image.clone(),
            vk::ImageAspectFlags::DEPTH));

        for render_mesh in &self.render_meshes {
            // create UBO for MVP
            let mvp_buffer = {
                let create_info = BufferCreateInfo::new(
                    vk::BufferCreateInfo::default()
                        .size(std::mem::size_of::<MVP>() as vk::DeviceSize)
                        .usage(vk::BufferUsageFlags::UNIFORM_BUFFER),
                    "MVP_buffer".to_string()
                );
                let buffer = device.create_buffer(
                    0, // TODO: create buffer handle
                    &create_info,
                    allocator.clone(),
                    MemoryLocation::CpuToGpu
                );

                device.update_buffer(&buffer, |mapped_memory: *mut c_void, _size: u64| {
                    let mvp = MVP {
                        model: render_mesh.transform.clone(),
                        view: self.camera.get_view(),
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

                Arc::new(Mutex::new(buffer))
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

            let dynamic_states = vec!(vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR);
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(std::slice::from_ref(&render_mesh.vertex_binding))
                .vertex_attribute_descriptions(&render_mesh.vertex_attributes);

            let pipeline_description = Arc::new(PipelineDescription::new(
                vec![render_mesh.vertex_binding.clone()],
                render_mesh.vertex_attributes.to_vec(),
                dynamic_states,
                RasterizationType::Standard,
                DepthStencilType::Enable,
                BlendType::None,
                "gltf-model-draw",
                self.vertex_shader.clone(),
                self.fragment_shader.clone()));

            let (viewport, scissor) = {
                let extent = back_buffer.resource_image.lock().unwrap().get_image().extent;
                let v = vk::Viewport::default()
                    .x(0.0)
                    // .y(0.0)
                    .y(extent.height as f32)
                    .width(extent.width as f32)
                    // .height(extent.height as f32)
                    .height(-(extent.height as f32))
                    .min_depth(0.0)
                    .max_depth(1.0);

                let s = vk::Rect2D::default()
                    .offset(vk::Offset2D{x: 0, y: 0})
                    .extent(vk::Extent2D{width: extent.width, height: extent.height});

                (v, s)
            };

            let albedo_binding = ResourceBinding {
                resource: render_mesh.albedo_tex.as_ref().unwrap().clone(),
                binding_info: BindingInfo {
                    binding_type: BindingType::Image(ImageBindingInfo {
                        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }),
                    set: 0,
                    slot: 1,
                    stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                    access: vk::AccessFlags::SHADER_READ,
                },
            };

            if let Some(ibo_ref) = &render_mesh.index_buffer {
                let ibo = ibo_ref.clone();
                let vbo = render_mesh.vertex_buffer.clone();
                let idx_length = render_mesh.num_indices;
                let passnode = GraphicsPassNode::builder("model_render".to_string())
                    .pipeline_description(pipeline_description)
                    .render_target(back_buffer.clone())
                    .depth_target(depth_attachment.clone())
                    .read(mvp_binding.clone())
                    .read(albedo_binding)
                    .tag(render_mesh.vertex_buffer.clone())
                    .tag(ibo.clone())
                    .viewport(viewport)
                    .scissor(scissor)
                    .fill_commands(Box::new(
                        move | device: DeviceInterface,
                               command_buffer: vk::CommandBuffer | {

                            enter_span!(tracing::Level::TRACE, "Draw RenderMesh");
                            enter_gpu_span!("RenderMesh GPU", "examples", device.get(), &command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);

                            unsafe {
                                enter_span!(tracing::Level::TRACE, "Model Draw");
                                // set vertex buffer
                                {
                                    if let ResourceType::Buffer(vb) = vbo.lock().unwrap().resource_type.as_ref().unwrap() {
                                        device.get().cmd_bind_vertex_buffers(
                                            command_buffer,
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
                                    if let ResourceType::Buffer(ib) = ibo.lock().unwrap().resource_type.as_ref().unwrap() {
                                        device.get().cmd_bind_index_buffer(
                                            command_buffer,
                                            ib.buffer,
                                            0 as vk::DeviceSize,
                                            vk::IndexType::UINT16
                                        );
                                    } else {
                                        panic!("Invalid index buffer for gltf draw");
                                    }
                                }

                                device.get().cmd_draw_indexed(
                                    command_buffer,
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

// The gltf lib returns transform matrices in column-major order as
// a &[[f32; 4];4]. Since I haven't found a glm::Mat4::from implementation
// which accepts that as an input, we use this utility function
fn gltf_to_glm(m: &[[f32; 4]; 4]) -> glm::Mat4 {
    glm::Mat4::from_columns(&[
        Vec4::from(m[0]),
        Vec4::from(m[1]),
        Vec4::from(m[2]),
        Vec4::from(m[3])
    ])
}

/// t is an owned Transform because Transform::decomposed takes self as an argument
fn gltf_to_decomposed_matrix(t: gltf::scene::Transform) -> DecomposedMatrix {
    let (translation, rotation, scale) = t.decomposed();
    // the gltf Transform specifies w (the scalar component of a quaternion) as the last element, but
    // nalgebra_glm expects it as the first argument to the Quaternion
    let rot_quat = glm::Quat::new(rotation[3], rotation[0], rotation[1], rotation[2]);
    DecomposedMatrix::new(
        glm::Vec3::new(translation[0], translation[1], translation[2]),
        glm::quat_to_mat4(&rot_quat),
        glm::Vec3::new(scale[0], scale[1], scale[2]))
}

impl ModelExample {
    pub fn new(
        device: DeviceInterface,
        render_context: &VulkanRenderContext,
        allocator: Arc<Mutex<ResourceAllocator>>,
        immediate_command_buffer: &vk::CommandBuffer) -> Self {

        let duck_import = gltf::import("assets/models/gltf/duck/Duck.gltf");
        // let duck_import = gltf::import("assets/models/gltf/Box/glTF/Box.gltf");
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
        let mut scene_cameras : Vec<Camera> = Vec::new();
        let mut meshes: Vec<RenderMesh> = Vec::new();
        for _scene in duck_gltf.document.scenes() {
            for node in duck_gltf.document.nodes() {
                let node_transform = gltf_to_glm(&node.transform().matrix());
                for child in node.children() {
                    let child_transform = gltf_to_glm(&child.transform().matrix());
                    if let Some(camera) = child.camera() {
                        match camera.projection() {
                            Projection::Orthographic(_ortho) => {
                                panic!("Currently don't support orthographic projections")
                            }
                            Projection::Perspective(persp) => {
                                // per the glTF 2.0 spec, we should exclude the scale of any node
                                // transforms in the camera's node hierarchy
                                // https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#view-matrix
                                let scene_resolved = {
                                    // Despite what the spec says, all glTF viewers I've found
                                    // online have included the scale components from their hierarchy
                                    node_transform
                                };
                                let view_resolved = scene_resolved.mul(&child_transform);

                                let far = match persp.zfar() {
                                    None => { 10000.0 }
                                    Some(zfar) => { zfar }
                                };
                                scene_cameras.push(Camera::new_from_view(
                                    persp.aspect_ratio().unwrap(),
                                    persp.yfov(),
                                    persp.znear(),
                                    far,
                                    view_resolved))
                            }
                        }
                    }
                    if let Some(mesh) = child.mesh() {
                        for (i, primitive) in mesh.primitives().enumerate() {
                            let mut ibo: Option<Arc<Mutex<DeviceResource>>> = None;

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
                                        vk::BufferCreateInfo::default()
                                            .size(ibo_size as vk::DeviceSize)
                                            .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                                            .sharing_mode(vk::SharingMode::EXCLUSIVE),
                                        primitive_name.clone()
                                    );
                                    Arc::new(Mutex::new(device.create_buffer(
                                        0, // TODO: create buffer handle
                                        &ibo_create,
                                        allocator.clone(),
                                        MemoryLocation::CpuToGpu
                                    )))
                                });

                                // * memory map the buffer
                                // * use the indices accessor to copy indices data into the GPU buffer
                                device.update_buffer(&ibo.as_ref().unwrap().lock().unwrap(), |mapped_memory: *mut c_void, _size: u64| {
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

                            // we want interior mutability of this map; i.e. we can't add or remove
                            // entries, but we can modify each existing entry
                            let vertex_attribute_map: HashMap<gltf::mesh::Semantic, RefCell<Option<gltf::Accessor>>> = HashMap::from([
                                (gltf::mesh::Semantic::Positions, RefCell::new(None)),
                                (gltf::mesh::Semantic::Normals, RefCell::new(None)),
                                (gltf::mesh::Semantic::TexCoords(0), RefCell::new(None))
                            ]);

                            let normals_offset = 3 * 4;
                            let uvs_offset = 3 * 4 + normals_offset;
                            let vertex_attributes: [vk::VertexInputAttributeDescription; 3] = [
                                // TODO: map the glTF componentTypes to the correct format (or alter the data)
                                // positions
                                vk::VertexInputAttributeDescription::default()
                                    .binding(0)
                                    .location(0)
                                    .format(vk::Format::R32G32B32_SFLOAT)
                                    .offset(0),

                                // normals
                                vk::VertexInputAttributeDescription::default()
                                    .binding(0)
                                    .location(1)
                                    .format(vk::Format::R32G32B32_SFLOAT)
                                    .offset(normals_offset),

                                // UVs
                                vk::VertexInputAttributeDescription::default()
                                    .binding(0)
                                    .location(2)
                                    .format(vk::Format::R32G32_SFLOAT)
                                    .offset(uvs_offset),
                            ];

                            // need to do an initial pass over attributes to calculate total VBO size and vertex size
                            let vertex_size = std::mem::size_of::<Vert>();
                            let mut vertex_data_size = 0usize;
                            let mut vertex_count = 0usize;
                            let mut found_positions = false;
                            for (semantic, attribute_accessor) in primitive.attributes() {
                                if semantic == gltf::mesh::Semantic::Positions {
                                    found_positions = true;

                                    vertex_count = attribute_accessor.count();
                                    vertex_data_size = vertex_size * vertex_count;
                                }
                                // only keep attributes which are used in the renderer
                                if let Some(found_attribute) = vertex_attribute_map.get(&semantic) {
                                    let mut attribute = found_attribute.borrow_mut();
                                    *attribute = Some(attribute_accessor);
                                }
                            }
                            assert!(found_positions, "No positions attribute was found while processing glTF model");

                            // create vertex buffer
                            let vbo = {
                                let vbo_create = BufferCreateInfo::new(
                                    vk::BufferCreateInfo::default()
                                        .size(vertex_data_size as vk::DeviceSize)
                                        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                                    primitive_name.clone()
                                );
                                device.create_buffer(
                                    0, // TODO: create buffer handle
                                    &vbo_create,
                                    allocator.clone(),
                                    MemoryLocation::CpuToGpu
                                )
                            };

                            let mut vertices : Vec<Vert> = Vec::new();
                            vertices.resize_with(vertex_count, Default::default);
                            // iterate over attributes again and copy them from mesh buffers into the VBO
                            for (semantic, attribute) in &vertex_attribute_map {
                                if let Some(attribute_accessor) = attribute.borrow().as_ref() {
                                    assert_eq!(
                                        attribute_accessor.count(),
                                        vertex_count,
                                        "Attribute count ({}) does not match vertex count ({})",
                                        attribute_accessor.count(),
                                        vertex_count);

                                    let view = attribute_accessor.view().expect("Failed to get view for vertex attribute");
                                    let buffer_data = duck_gltf.buffers.get(view.buffer().index())
                                        .expect("Failed to get buffer for vertex attribute");
                                    let stride = match view.stride() {
                                        None => {1} // I think this is a safe assumption?
                                        Some(s) => {s}
                                    };

                                    // source_offset is the offset into the source buffer defined by the buffer view (base) and the accessor
                                    let mut source_offset = view.offset() + attribute_accessor.offset();

                                    for i in (0..vertex_count) {
                                        let vertex = vertices.get_mut(i).unwrap();

                                        let glm_value = unsafe {
                                            get_glm_format(
                                                attribute_accessor.data_type(),
                                                attribute_accessor.dimensions(),
                                                buffer_data.0.as_ptr().byte_add(source_offset))
                                        };

                                        match semantic {
                                            Semantic::Positions => {
                                                let GlmType::Vec3(pos) = glm_value else {
                                                    panic!("Position must be a vec3")
                                                };
                                                vertex.pos = [pos.x, pos.y, pos.z];
                                            }
                                            Semantic::Normals => {
                                                let GlmType::Vec3(normal) = glm_value else {
                                                    panic!("Normals must be a vec3")
                                                };
                                                vertex.normal = [normal.x, normal.y, normal.z];
                                            }
                                            Semantic::TexCoords(0) => {
                                                let GlmType::Vec2(uv) = glm_value else {
                                                    panic!("UVs must be a vec2")
                                                };
                                                vertex.uv = [uv.x, uv.y];
                                            }
                                            _ => {
                                                panic!("Unsupported input semantic");
                                            }
                                        }

                                        source_offset += stride;
                                    }
                                } else {
                                    // use default values
                                    for i in (0..vertex_count) {
                                        let vertex = vertices.get_mut(i).unwrap();

                                        match semantic {
                                            gltf::Semantic::Normals => {
                                                // TODO: we should actually calculate this based on neighboring vertex positions
                                                vertex.normal = [0.0, 0.0, 1.0];
                                            },
                                            gltf::Semantic::TexCoords(0) => {
                                                vertex.uv = [0.0, 0.0];
                                            },
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            device.update_buffer(&vbo, |mapped_memory: *mut c_void, _size: u64| {
                                unsafe {
                                    // core::ptr::copy_nonoverlapping(
                                    //     vertices.as_ptr(),
                                    //     mapped_memory as *mut Vert,
                                    //     vertices.len());
                                    for i in (0..vertex_count) {
                                        let vertex = &vertices[i];

                                        let offset = i * std::mem::size_of::<Vert>();

                                        core::ptr::copy_nonoverlapping(
                                            &vertex.pos,
                                            (mapped_memory as *mut [f32;3]).byte_add(offset),
                                            1);

                                        core::ptr::copy_nonoverlapping(
                                            &vertex.normal,
                                            (mapped_memory as *mut [f32;3]).byte_add(offset + (3*4)),
                                            1);

                                        core::ptr::copy_nonoverlapping(
                                            &vertex.uv,
                                            (mapped_memory as *mut [f32;2]).byte_add(offset + (6*4)),
                                            1);
                                    }
                                }
                            });

                            // process  material
                            let mut albedo_dev_tex: Option<Arc<Mutex<DeviceResource>>> = None;
                            {
                                let material = primitive.material();
                                if let Some(material_id) = material.index() {
                                    if let Some(albedo_tex) = material.pbr_metallic_roughness().base_color_texture() {
                                        // create device image from image bytes
                                        let image_source = albedo_tex.texture().source().source();
                                        match image_source {
                                            Source::View{view, mime_type } => {
                                                let buffer_data = duck_gltf.buffers.get(view.buffer().index())
                                                    .expect("Failed to get buffer data for image");
                                                let source_offset = view.offset();
                                                // util::image::create_from_bytes(
                                                //     device.clone(),
                                                //     render_context)

                                                // let view = indices_accessor.view().expect("Failed to get view for index buffer");
                                                // let buffer_data = duck_gltf.buffers.get(view.buffer().index())
                                                //     .expect("Failed to get buffer data for index buffer");
                                                // let source_offset = view.offset() + indices_accessor.offset();
                                                // core::ptr::copy_nonoverlapping(
                                                //     buffer_data.0.as_ptr().byte_add(source_offset),
                                                //     mapped_memory as *mut u8,
                                                //     ibo_size);
                                            }
                                            Source::Uri{ uri, mime_type } => {
                                                let mut tex = util::image::create_from_uri(
                                                    0, // TODO: create image handle
                                                    device.clone(),
                                                    allocator.clone(),
                                                    immediate_command_buffer,
                                                    render_context.get_graphics_queue_index(),
                                                    render_context.get_graphics_queue(),
                                                    &format!("{}{}", "assets/models/gltf/duck/", uri),
                                                    true
                                                );
                                                // albedo_dev_tex = Some(Rc::new(RefCell::new(tex)));
                                                unsafe {
                                                    let create = vk::SamplerCreateInfo::default()
                                                        .mag_filter(vk::Filter::LINEAR)
                                                        .min_filter(vk::Filter::LINEAR)
                                                        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                                                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                                                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                                                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                                                        .border_color(vk::BorderColor::INT_OPAQUE_BLACK);

                                                    let sampler = device.get().create_sampler(&create, None)
                                                        .expect("Failed to create sampler for albedo texture");
                                                    device.set_debug_name(sampler, "albedo_sampler");

                                                    tex.get_image_mut().sampler = Some(sampler);
                                                };
                                                albedo_dev_tex = Some(Arc::new(Mutex::new(tex)));
                                            }
                                        }

                                        // create sampler

                                        // apply sampler to device image
                                    }
                                }
                            }

                            let render_mesh = RenderMesh {
                                vertex_buffer: Arc::new(Mutex::new(vbo)),
                                index_buffer: ibo,
                                num_indices,
                                vertex_binding: VERTEX_BINDING,
                                vertex_attributes,
                                transform: node_transform.mul(child_transform),
                                albedo_tex: albedo_dev_tex,
                            };
                            meshes.push(render_mesh);
                        }
                    }
                }
            }
        }

        // let camera = Camera::new(
        //     1.5,
        //     0.66,
        //     1.0,
        //     10000.0,
        //     &glm::Vec3::new(0.0, 0.0, 100.0),
        //     &glm::Vec3::new(0.0, 0.0, -1.0),
        //     &glm::Vec3::new(0.0, 1.0, 0.0)
        // );

        let camera = {
            if scene_cameras.len() > 0 {
                scene_cameras[0].clone()
            } else {
                Camera::new_from_view(
                    1.5,
                    0.66,
                    1.0,
                    10000.0,
                    glm::look_at(
                        &glm::Vec3::new(0.0, 0.0, 2.0),
                        &glm::Vec3::new(0.0, 0.0, -1.0),
                        &glm::Vec3::new(0.0, 1.0, 0.0)
                    ).try_inverse().unwrap()
                    // glm::Mat4::from_columns(&[
                    //     Vec4::new( -0.7289686799049377, 0.0, -0.6845470666885376, 0.0),
                    //     Vec4::new(-0.4252049028873444, 0.7836934328079224, 0.4527972936630249, 0.0),
                    //     Vec4::new(0.5364750623703003, 0.6211478114128113, -0.571287989616394, 0.0),
                    //     Vec4::new( 400.1130065917969, 463.2640075683594, -431.0780334472656, 1.00)
                    // ])
                )
            }

        };

        // let camera = Camera::new_from_view(
        //     1.5,
        //     0.66,
        //     1.0,
        //     10000.0,
        //     glm::Mat4::from_columns(&[
        //         Vec4::new( -0.7289686799049377, -0.425, 0.5365, 400.113),
        //         Vec4::new(0.0, 0.7836934328079224, 0.62115, 463.264),
        //         Vec4::new(-0.6845, 0.4528, -0.571287989616394, -431.078),
        //         Vec4::new( 0.0, 0.0, 0.0, 1.00)
        //     ])
        // );

        // glm::Mat4::from_columns(&[
        //     Vec4::from(m[0]),
        //     Vec4::from(m[1]),
        //     Vec4::from(m[2]),
        //     Vec4::from(m[3])
        // ])

        let vert_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "model-vert",
                include_bytes!(concat!(env!("OUT_DIR"), "/shaders/model-vert.spv")))));
        let frag_shader = Arc::new(Mutex::new(
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