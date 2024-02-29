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
use context::api_types::device::{DeviceResource, DeviceWrapper};
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;
use once_cell::sync::Lazy;
use crate::example::Example;
use crate::ubo_example::UBO;

pub struct GltfModel {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>
}

pub struct RenderMesh {
    vertex_buffer: Rc<RefCell<DeviceResource>>,
    index_buffer: Option<Rc<RefCell<DeviceResource>>>

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
    duck_model: GltfModel,
    render_mesh: RenderMesh
}

impl Example for ModelExample {
    fn get_name(&self) -> &'static str {
        "Model Render"
    }

    fn execute(&self, imgui_ui: &mut Ui, back_buffer: AttachmentReference) -> Vec<PassType> {
        Vec::new()
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
        let mut vbo: Option<DeviceResource> = None;
        let mut ibo: Option<Rc<RefCell<DeviceResource>>> = None;
        for node in duck_gltf.document.nodes() {
            if let Some(mesh) = node.mesh() {
                for (i, primitive) in mesh.primitives().enumerate() {
                    let primitive_name = {
                        if let Some(mesh_name) = mesh.name() {
                            format!("{}_{}", mesh_name, i)
                        } else {
                            format!("UnknownMesh_{}", i)
                        }
                    };

                    let mode = primitive.mode();
                    if let Some(indices_accessor) = primitive.indices() {
                        // * create GPU index buffer
                        let ibo_size = indices_accessor.count() * indices_accessor.size();
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
                                core::ptr::copy_nonoverlapping(
                                    buffer_data.0.as_ptr(),
                                    mapped_memory as *mut u8,
                                    1);
                            }
                        });
                    }

                    let mut vertex_data_size = 0usize;
                    let mut vertex_size = 0usize;
                    let mut vertex_attributes: Vec<vk::VertexInputAttributeDescription> = Vec::new();
                    // need to do an initial pass over attributes to calculate total VBO size and vertex size
                    for (_, attribute_accessor) in primitive.attributes() {
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
                            .location(attribute_accessor.index() as u32) // TODO: should match this to shader via reflection instead?
                            .offset(offset as u32)
                            .build()
                        );
                    }

                    let vertex_binding = vk::VertexInputBindingDescription::builder()
                        .binding(0)
                        .input_rate(vk::VertexInputRate::VERTEX)
                        .stride(vertex_size as u32)
                        .build();

                    // create vertex buffer
                    vbo = {
                        let vbo_create = BufferCreateInfo::new(
                            vk::BufferCreateInfo::builder()
                                .size(vertex_data_size as vk::DeviceSize)
                                .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                .build(),
                            primitive_name.clone()
                        );
                        Some(DeviceWrapper::create_buffer(
                            device.clone(),
                            &vbo_create,
                            MemoryLocation::CpuToGpu
                        ))
                    };

                    // iterate over attributes again and copy them from mesh buffers into the VBO
                    device.borrow().update_buffer(vbo.as_ref().unwrap(), |mapped_memory: *mut c_void, _size: u64| {
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
                                        buffer_data.0.as_ptr().add(source_offset),
                                        mapped_memory as *mut u8,
                                        1);

                                    source_offset += stride;
                                    dest_offset += vertex_size;
                                }

                                vertex_offset += attribute_accessor.size();
                            }
                        }
                    });
                }
            }
        }

        ModelExample{
            duck_model: duck_gltf,
            render_mesh: RenderMesh {
                vertex_buffer: Rc::new(RefCell::new(vbo.expect("No VBO created"))),
                index_buffer: ibo,
            }
        }
    }
}