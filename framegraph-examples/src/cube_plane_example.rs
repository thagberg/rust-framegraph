use core::ffi::c_void;
use std::sync::{Arc, Mutex};
use ash::vk;
use gpu_allocator::MemoryLocation;
use imgui::Ui;
use api_types::buffer::BufferCreateInfo;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::resource::{DeviceResource, ResourceType};
use api_types::device::interface::DeviceInterface;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ImageBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader::{self, Shader};
use profiling::enter_span;
use crate::example::Example;
use nalgebra_glm as glm;
use passes::clear;
use passes::shadow::{ShadowMappingObject, ShadowPass, LightMVP};
use util::camera::Camera;

#[repr(C)]
struct SceneUniforms {
    view: glm::Mat4,
    proj: glm::Mat4,
    light_pos: glm::Vec4,
    view_pos: glm::Vec4,
    light_dir: glm::Vec4,
    spotlight_angle: f32,
    _pad0: [f32; 3],
}

#[repr(C)]
struct ModelUniforms {
    model: glm::Mat4,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

pub struct CubePlaneExample {
    scene_ubo: Arc<Mutex<DeviceResource>>,
    cube_ubo: Arc<Mutex<DeviceResource>>,
    plane_ubo: Arc<Mutex<DeviceResource>>,
    cube_vbuf: Arc<Mutex<DeviceResource>>,
    cube_ibuf: Arc<Mutex<DeviceResource>>,
    plane_vbuf: Arc<Mutex<DeviceResource>>,
    plane_ibuf: Arc<Mutex<DeviceResource>>,
    vertex_shader: Arc<Mutex<Shader>>,
    fragment_shader: Arc<Mutex<Shader>>,
    time: f32,
    light_angle: f32,
    camera_distance: f32,
    spotlight_opening_angle: f32,
}

impl Example for CubePlaneExample {
    fn get_name(&self) -> &'static str {
        "Cube & Plane"
    }

    fn execute(
        &self,
        device: DeviceInterface,
        allocator: Arc<Mutex<ResourceAllocator>>,
        imgui_ui: &mut Ui,
        back_buffer: AttachmentReference) -> Vec<PassType> {

        // Update Scene Uniforms
        let extent = back_buffer.resource_image.lock().unwrap().get_image().extent;
        let width = extent.width as f32;
        let height = extent.height as f32;

        let target = glm::vec3(0.0, 0.5, 0.0);
        let base_view_dir = glm::normalize(&(glm::vec3(0.0, 3.0, 6.0) - target));
        let view_pos = target + base_view_dir * self.camera_distance;
        
        let camera = Camera::new(
            width / height,
            45.0f32.to_radians(),
            0.1,
            100.0,
            &view_pos,
            &target,
            &glm::vec3(0.0, 1.0, 0.0)
        );

        let light_radius = 5.0f32;
        let light_x = self.light_angle.cos() * light_radius;
        let light_z = self.light_angle.sin() * light_radius;
        let light_pos = glm::vec4(light_x, 5.0, light_z, 1.0);
        let light_dir = glm::normalize(&(target - light_pos.xyz()));

        let uniforms = SceneUniforms {
            view: camera.view,
            proj: camera.projection,
            light_pos,
            view_pos: glm::vec4(view_pos.x, view_pos.y, view_pos.z, 1.0),
            light_dir: glm::vec4(light_dir.x, light_dir.y, light_dir.z, 0.0),
            spotlight_angle: self.spotlight_opening_angle.cos(),
            _pad0: [0.0; 3],
        };

        device.update_buffer(&self.scene_ubo.lock().unwrap(), |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &uniforms,
                    mapped_memory as *mut SceneUniforms,
                    1);
            }
        });

        // Update Cube Uniforms
        let y_pos = 1.0 + (self.time * 2.0).sin() * 0.5;
        let cube_model_mat = glm::translate(&glm::Mat4::identity(), &glm::vec3(0.0, y_pos, 0.0));
        let cube_model_mat = glm::rotate(&cube_model_mat, self.time, &glm::vec3(0.0, 1.0, 0.0));
        
        device.update_buffer(&self.cube_ubo.lock().unwrap(), |mapped_memory: *mut c_void, _size: u64| {
            let cube_uniforms = ModelUniforms { model: cube_model_mat };
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &cube_uniforms,
                    mapped_memory as *mut ModelUniforms,
                    1);
            }
        });

        // Update Plane Uniforms (Static)
        device.update_buffer(&self.plane_ubo.lock().unwrap(), |mapped_memory: *mut c_void, _size: u64| {
            let plane_uniforms = ModelUniforms { model: glm::Mat4::identity() };
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &plane_uniforms,
                    mapped_memory as *mut ModelUniforms,
                    1);
            }
        });

        imgui_ui.text(format!("Cube Floating over Plane"));
        imgui_ui.text(format!("Time: {:.2}", self.time));

        let mut angle = self.light_angle;
        imgui_ui.slider("Light Rotation", 0.0, 2.0 * std::f32::consts::PI, &mut angle);
        
        let mut spotlight_angle = self.spotlight_opening_angle.to_degrees();
        imgui_ui.slider("Spotlight Angle", 1.0, 90.0, &mut spotlight_angle);

        unsafe {
            let mut_self = self as *const Self as *mut Self;
            (*mut_self).light_angle = angle;
            (*mut_self).spotlight_opening_angle = spotlight_angle.to_radians();
        }

        let mut passes: Vec<PassType> = Vec::new();

        // Create Depth Target
        let depth_attachment = {
            let depth_image = {
                let depth_create = vk::ImageCreateInfo::default()
                    .format(vk::Format::D32_SFLOAT)
                    .image_type(vk::ImageType::TYPE_2D)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
                    .extent(extent)
                    .mip_levels(1)
                    .array_layers(1);

                let image_create = api_types::image::ImageCreateInfo::new(
                    depth_create,
                    "cube_plane_depth".to_string(),
                    api_types::image::ImageType::Depth
                );

                device.create_image(
                    0,
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

        // Clear Depth Target
        passes.push(clear::clear(
            depth_attachment.resource_image.clone(),
            vk::ImageAspectFlags::DEPTH));

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
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(24),
        ];

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let pipeline_description = Arc::new(PipelineDescription::new(
            vec![vertex_binding],
            vertex_attributes.clone(),
            dynamic_states,
            RasterizationType::CullBack,
            DepthStencilType::Enable,
            BlendType::None,
            "cube_plane",
            self.vertex_shader.clone(),
            self.fragment_shader.clone()));

        let scene_binding = ResourceBinding {
            resource: self.scene_ubo.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo {
                    offset: 0,
                    range: std::mem::size_of::<SceneUniforms>() as vk::DeviceSize
                }),
                set: 0,
                slot: 0,
                stage: vk::PipelineStageFlags::ALL_GRAPHICS,
                access: vk::AccessFlags::SHADER_READ
            },
        };

        let cube_ubo_binding = ResourceBinding {
            resource: self.cube_ubo.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo {
                    offset: 0,
                    range: std::mem::size_of::<ModelUniforms>() as vk::DeviceSize
                }),
                set: 0,
                slot: 1,
                stage: vk::PipelineStageFlags::VERTEX_SHADER,
                access: vk::AccessFlags::SHADER_READ
            },
        };

        let plane_ubo_binding = ResourceBinding {
            resource: self.plane_ubo.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo {
                    offset: 0,
                    range: std::mem::size_of::<ModelUniforms>() as vk::DeviceSize
                }),
                set: 0,
                slot: 1,
                stage: vk::PipelineStageFlags::VERTEX_SHADER,
                access: vk::AccessFlags::SHADER_READ
            },
        };

        let cube_vbuf_ref = self.cube_vbuf.clone();
        let cube_ibuf_ref = self.cube_ibuf.clone();
        let plane_vbuf_ref = self.plane_vbuf.clone();
        let plane_ibuf_ref = self.plane_ibuf.clone();

        let (viewport, scissor) = {
            let v = vk::Viewport::default()
                .x(0.0)
                .y(height)
                .width(width)
                .height(-height)
                .min_depth(0.0)
                .max_depth(1.0);

            let s = vk::Rect2D::default()
                .offset(vk::Offset2D { x: 0, y: 0 })
                .extent(vk::Extent2D { width: width as u32, height: height as u32 });

            (v, s)
        };

        // create shadow map and attachment
        let shadow_map = {
            let sm_create = vk::ImageCreateInfo::default()
                .format(vk::Format::D32_SFLOAT)
                .image_type(vk::ImageType::TYPE_2D)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .extent(vk::Extent3D { width: 1024, height: 1024, depth: 1 })
                .mip_levels(1)
                .array_layers(1);

            let image_create = api_types::image::ImageCreateInfo::new(
                sm_create,
                "shadow_map".to_string(),
                api_types::image::ImageType::Depth
            );

            Arc::new(Mutex::new(device.create_image(
                0,
                &image_create,
                allocator.clone(),
                MemoryLocation::GpuOnly
            )))
        };

        unsafe {
            let sampler_create = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE);

            let sampler = device.get().create_sampler(&sampler_create, None)
                .expect("Failed to create sampler for shadow map");
            device.set_debug_name(sampler, "shadow_sampler");

            shadow_map.lock().unwrap().get_image_mut().sampler = Some(sampler);
        }

        let shadow_attachment = AttachmentReference::new(
            shadow_map.clone(),
            vk::SampleCountFlags::TYPE_1);

        // create shadow objects vector
        let shadow_objects = vec![
            // cube object
            ShadowMappingObject{
                vertex_buffer: cube_vbuf_ref.clone(),
                index_buffer: cube_ibuf_ref.clone(),
                index_count: 36, // TODO: should store this somewhere
                vertex_binding,
                vertex_attributes: vertex_attributes.clone(), // TODO: can these be by ref?
                model_matrix: cube_model_mat
            },
            ShadowMappingObject{
                vertex_buffer: plane_vbuf_ref.clone(),
                index_buffer: plane_ibuf_ref.clone(),
                index_count: 6,
                vertex_binding,
                vertex_attributes,
                model_matrix: glm::Mat4::identity()
            }
        ];

        // create light UBO
        let shadow_camera = Camera::new(
            1.0,
            self.light_angle,
            0.1,
            100.0,
            &light_dir,
            &light_pos.xyz(),
            &glm::vec3(0.0, 1.0, 0.0));

        let shadow_ubo = create_ubo(
            device.clone(),
            allocator.clone(),
            std::mem::size_of::<LightMVP>() as u64,
            "shadow_ubo");


        let light_mvp = LightMVP{
            view: shadow_camera.view,
            proj: shadow_camera.projection
        };

        device.update_buffer(&shadow_ubo, |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &light_mvp,
                    mapped_memory as *mut LightMVP,
                    1
                );
            }
        });

        let shadows = ShadowPass::new(device.clone());
        let shadow_pass = shadows.generate_pass(
            shadow_attachment,
            shadow_objects,
            Arc::new(Mutex::new(shadow_ubo)));

        // create binding for shadow map
        let shadow_binding = ResourceBinding {
            resource: shadow_map.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Image(ImageBindingInfo{
                    layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }),
                set: 0,
                slot: 2,
                stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                access: vk::AccessFlags::SHADER_READ,
            },
        };

        // Pass for Plane
        let plane_pass = GraphicsPassNode::builder("plane_pass".to_string())
            .pipeline_description(pipeline_description.clone())
            .read(scene_binding.clone())
            .read(plane_ubo_binding)
            .read(shadow_binding.clone())
            .render_target(back_buffer.clone())
            .depth_target(depth_attachment.clone())
            .viewport(viewport)
            .scissor(scissor)
            .fill_commands(Box::new(
                move |device: DeviceInterface,
                      command_buffer: vk::CommandBuffer,
                      _pipeline_layout: vk::PipelineLayout| {
                    enter_span!(tracing::Level::TRACE, "Draw Plane");
                    unsafe {
                        let vbuf = plane_vbuf_ref.lock().unwrap();
                        let ibuf = plane_ibuf_ref.lock().unwrap();
                        if let (ResourceType::Buffer(vb), ResourceType::Buffer(ib)) = (vbuf.resource_type.as_ref().unwrap(), ibuf.resource_type.as_ref().unwrap()) {
                            device.get().cmd_bind_vertex_buffers(command_buffer, 0, &[vb.buffer], &[0]);
                            device.get().cmd_bind_index_buffer(command_buffer, ib.buffer, 0, vk::IndexType::UINT32);
                        }
                        device.get().cmd_draw_indexed(command_buffer, 6, 1, 0, 0, 0);
                    }
                }
            ))
            .build()
            .expect("Failed to create Plane passnode");

        // Pass for Cube
        let cube_pass = GraphicsPassNode::builder("cube_pass".to_string())
            .pipeline_description(pipeline_description)
            .read(scene_binding)
            .read(cube_ubo_binding)
            .read(shadow_binding.clone())
            .render_target(back_buffer)
            .depth_target(depth_attachment)
            .viewport(viewport)
            .scissor(scissor)
            .fill_commands(Box::new(
                move |device: DeviceInterface,
                      command_buffer: vk::CommandBuffer,
                      _pipeline_layout: vk::PipelineLayout| {
                    enter_span!(tracing::Level::TRACE, "Draw Cube");
                    unsafe {
                        let vbuf = cube_vbuf_ref.lock().unwrap();
                        let ibuf = cube_ibuf_ref.lock().unwrap();
                        if let (ResourceType::Buffer(vb), ResourceType::Buffer(ib)) = (vbuf.resource_type.as_ref().unwrap(), ibuf.resource_type.as_ref().unwrap()) {
                            device.get().cmd_bind_vertex_buffers(command_buffer, 0, &[vb.buffer], &[0]);
                            device.get().cmd_bind_index_buffer(command_buffer, ib.buffer, 0, vk::IndexType::UINT32);
                        }
                        device.get().cmd_draw_indexed(command_buffer, 36, 1, 0, 0, 0);
                    }
                }
            ))
            .build()
            .expect("Failed to create Cube passnode");

        passes.push(shadow_pass);
        passes.push(PassType::Graphics(plane_pass));
        passes.push(PassType::Graphics(cube_pass));
        passes
    }

    fn update(&mut self, delta_time: f32) {
        self.time += delta_time;
    }

    fn handle_event(&mut self, event: &winit::event::Event<()>) {
        use winit::event::{WindowEvent, MouseScrollDelta, Event};
        if let Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } = event {
            let zoom_amount = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(pos) => (pos.y as f32) / 20.0,
            };
            self.camera_distance = (self.camera_distance - zoom_amount).clamp(1.0, 50.0);
        }
    }
}

impl CubePlaneExample {
    pub fn new(
        device: DeviceInterface,
        _render_context: &VulkanRenderContext,
        allocator: Arc<Mutex<ResourceAllocator>>) -> Self {

        let (cube_vertices, cube_indices) = create_cube_data();
        let (plane_vertices, plane_indices) = create_plane_data();

        let cube_vbuf = create_vertex_buffer(device.clone(), allocator.clone(), &cube_vertices, "cube_vbuf");
        let cube_ibuf = create_index_buffer(device.clone(), allocator.clone(), &cube_indices, "cube_ibuf");
        let plane_vbuf = create_vertex_buffer(device.clone(), allocator.clone(), &plane_vertices, "plane_vbuf");
        let plane_ibuf = create_index_buffer(device.clone(), allocator.clone(), &plane_indices, "plane_ibuf");

        let scene_ubo = create_ubo(device.clone(), allocator.clone(), std::mem::size_of::<SceneUniforms>() as u64, "scene_ubo");
        let cube_ubo = create_ubo(device.clone(), allocator.clone(), std::mem::size_of::<ModelUniforms>() as u64, "cube_ubo");
        let plane_ubo = create_ubo(device.clone(), allocator.clone(), std::mem::size_of::<ModelUniforms>() as u64, "plane_ubo");

        let vertex_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "cube_plane-vert",
                include_bytes!(concat!(env!("SHADER_DIR"), "/cube_plane-vert.spv")))));
        
        let fragment_shader = Arc::new(Mutex::new(
            shader::create_shader_module_from_bytes(
                device.clone(),
                "cube_plane-frag",
                include_bytes!(concat!(env!("SHADER_DIR"), "/cube_plane-frag.spv")))));

        CubePlaneExample {
            scene_ubo: Arc::new(Mutex::new(scene_ubo)),
            cube_ubo: Arc::new(Mutex::new(cube_ubo)),
            plane_ubo: Arc::new(Mutex::new(plane_ubo)),
            cube_vbuf: Arc::new(Mutex::new(cube_vbuf)),
            cube_ibuf: Arc::new(Mutex::new(cube_ibuf)),
            plane_vbuf: Arc::new(Mutex::new(plane_vbuf)),
            plane_ibuf: Arc::new(Mutex::new(plane_ibuf)),
            vertex_shader,
            fragment_shader,
            time: 0.0,
            light_angle: 45.0f32.to_radians(),
            camera_distance: 6.0,
            spotlight_opening_angle: 30.0f32.to_radians(),
        }
    }
}

fn create_ubo(device: DeviceInterface, allocator: Arc<Mutex<ResourceAllocator>>, size: u64, name: &str) -> DeviceResource {
    let ubo_create = BufferCreateInfo::new(
        vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE),
        name.to_string()
    );

    device.create_buffer(0, &ubo_create, allocator, MemoryLocation::CpuToGpu)
}

fn create_vertex_buffer(device: DeviceInterface, allocator: Arc<Mutex<ResourceAllocator>>, vertices: &[Vertex], name: &str) -> DeviceResource {
    let size = (std::mem::size_of::<Vertex>() * vertices.len()) as vk::DeviceSize;
    let create_info = BufferCreateInfo::new(
        vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE),
        name.to_string()
    );

    let buffer = device.create_buffer(0, &create_info, allocator, MemoryLocation::CpuToGpu);
    device.update_buffer(&buffer, |mapped_memory: *mut c_void, _size: u64| {
        unsafe {
            core::ptr::copy_nonoverlapping(vertices.as_ptr(), mapped_memory as *mut Vertex, vertices.len());
        }
    });
    buffer
}

fn create_index_buffer(device: DeviceInterface, allocator: Arc<Mutex<ResourceAllocator>>, indices: &[u32], name: &str) -> DeviceResource {
    let size = (std::mem::size_of::<u32>() * indices.len()) as vk::DeviceSize;
    let create_info = BufferCreateInfo::new(
        vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE),
        name.to_string()
    );

    let buffer = device.create_buffer(0, &create_info, allocator, MemoryLocation::CpuToGpu);
    device.update_buffer(&buffer, |mapped_memory: *mut c_void, _size: u64| {
        unsafe {
            core::ptr::copy_nonoverlapping(indices.as_ptr(), mapped_memory as *mut u32, indices.len());
        }
    });
    buffer
}

fn create_cube_data() -> (Vec<Vertex>, Vec<u32>) {
    let color = [0.8, 0.2, 0.3];
    let vertices = vec![
        // Front
        Vertex { pos: [-0.5, -0.5,  0.5], normal: [ 0.0,  0.0,  1.0], color }, // 0
        Vertex { pos: [ 0.5, -0.5,  0.5], normal: [ 0.0,  0.0,  1.0], color }, // 1
        Vertex { pos: [ 0.5,  0.5,  0.5], normal: [ 0.0,  0.0,  1.0], color }, // 2
        Vertex { pos: [-0.5,  0.5,  0.5], normal: [ 0.0,  0.0,  1.0], color }, // 3
        // Back
        Vertex { pos: [-0.5, -0.5, -0.5], normal: [ 0.0,  0.0, -1.0], color }, // 4
        Vertex { pos: [ 0.5, -0.5, -0.5], normal: [ 0.0,  0.0, -1.0], color }, // 5
        Vertex { pos: [ 0.5,  0.5, -0.5], normal: [ 0.0,  0.0, -1.0], color }, // 6
        Vertex { pos: [-0.5,  0.5, -0.5], normal: [ 0.0,  0.0, -1.0], color }, // 7
        // Top
        Vertex { pos: [-0.5,  0.5,  0.5], normal: [ 0.0,  1.0,  0.0], color }, // 8
        Vertex { pos: [ 0.5,  0.5,  0.5], normal: [ 0.0,  1.0,  0.0], color }, // 9
        Vertex { pos: [ 0.5,  0.5, -0.5], normal: [ 0.0,  1.0,  0.0], color }, // 10
        Vertex { pos: [-0.5,  0.5, -0.5], normal: [ 0.0,  1.0,  0.0], color }, // 11
        // Bottom
        Vertex { pos: [-0.5, -0.5,  0.5], normal: [ 0.0, -1.0,  0.0], color }, // 12
        Vertex { pos: [ 0.5, -0.5,  0.5], normal: [ 0.0, -1.0,  0.0], color }, // 13
        Vertex { pos: [ 0.5, -0.5, -0.5], normal: [ 0.0, -1.0,  0.0], color }, // 14
        Vertex { pos: [-0.5, -0.5, -0.5], normal: [ 0.0, -1.0,  0.0], color }, // 15
        // Right
        Vertex { pos: [ 0.5, -0.5,  0.5], normal: [ 1.0,  0.0,  0.0], color }, // 16
        Vertex { pos: [ 0.5,  0.5,  0.5], normal: [ 1.0,  0.0,  0.0], color }, // 17
        Vertex { pos: [ 0.5,  0.5, -0.5], normal: [ 1.0,  0.0,  0.0], color }, // 18
        Vertex { pos: [ 0.5, -0.5, -0.5], normal: [ 1.0,  0.0,  0.0], color }, // 19
        // Left
        Vertex { pos: [-0.5, -0.5,  0.5], normal: [-1.0,  0.0,  0.0], color }, // 20
        Vertex { pos: [-0.5,  0.5,  0.5], normal: [-1.0,  0.0,  0.0], color }, // 21
        Vertex { pos: [-0.5,  0.5, -0.5], normal: [-1.0,  0.0,  0.0], color }, // 22
        Vertex { pos: [-0.5, -0.5, -0.5], normal: [-1.0,  0.0,  0.0], color }, // 23
    ];

    let indices = vec![
        0, 3, 2, 2, 1, 0, // Front
        4, 5, 6, 6, 7, 4, // Back
        8, 11, 10, 10, 9, 8, // Top
        12, 13, 14, 14, 15, 12, // Bottom
        16, 17, 18, 18, 19, 16, // Right
        20, 23, 22, 22, 21, 20, // Left
    ];

    (vertices, indices)
}

fn create_plane_data() -> (Vec<Vertex>, Vec<u32>) {
    let color = [0.3, 0.5, 0.3];
    let n = [0.0, 1.0, 0.0];
    let s = 5.0;
    let vertices = vec![
        Vertex { pos: [-s, 0.0,  s], normal: n, color }, // 0
        Vertex { pos: [ s, 0.0,  s], normal: n, color }, // 1
        Vertex { pos: [ s, 0.0, -s], normal: n, color }, // 2
        Vertex { pos: [-s, 0.0, -s], normal: n, color }, // 3
    ];

    let indices = vec![
        0, 3, 2, 2, 1, 0
    ];

    (vertices, indices)
}
