use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use ash::vk;
use ash::vk::{DeviceSize, Handle};
use gpu_allocator::MemoryLocation;
use imgui::{DrawData, DrawVert, DrawIdx};
use api_types::buffer::BufferCreateInfo;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::resource::{DeviceResource, ResourceType};
use api_types::device::interface::DeviceInterface;

use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ImageBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use framegraph::shader::Shader;
use profiling::{enter_gpu_span, enter_span};
use util::image;

const IMGUI_VERTEX_BINDING: vk::VertexInputBindingDescription = vk::VertexInputBindingDescription{
    binding: 0,
    stride: std::mem::size_of::<DrawVert>() as u32,
    input_rate: vk::VertexInputRate::VERTEX,
};

const IMGUI_VERTEX_ATTRIBUTES: [vk::VertexInputAttributeDescription; 3] = [
    // pos
    vk::VertexInputAttributeDescription {
        location: 0,
        binding: 0,
        format: vk::Format::R32G32_SFLOAT,
        offset: 0,
    },

    // uv
    vk::VertexInputAttributeDescription {
        location: 1,
        binding: 0,
        format: vk::Format::R32G32_SFLOAT,
        offset: 4 * 2,
    },

    // color
    vk::VertexInputAttributeDescription {
        location: 2,
        binding: 0,
        format: vk::Format::R8G8B8A8_UNORM,
        offset: 4 * 4,
    }
];

pub struct DisplayBuffer {
    scale: [f32; 2],
    pos: [f32; 2]
}

pub struct ImguiRender<'d> {
    vertex_shader: Rc<RefCell<Shader<'d>>>,
    fragment_shader: Rc<RefCell<Shader<'d>>>,
    font_texture: Arc<Mutex<DeviceResource<'d>>>
}

impl<'d> Debug for ImguiRender<'d> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImguiRender")
            .finish()
    }
}

impl<'d> Drop for ImguiRender<'d> {
    fn drop(&mut self) {
        println!("Dropping ImguiRender");
    }
}

impl<'d> ImguiRender<'d> {
    pub fn new(
        device: &'d DeviceInterface,
        render_context: &VulkanRenderContext,
        allocator: Arc<Mutex<ResourceAllocator>>,
        immediate_command_buffer: &vk::CommandBuffer,
        font_atlas: imgui::FontAtlasTexture) -> ImguiRender<'d> {

        let vert_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(device, "imgui-vert", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/imgui-vert.spv")))));
        let frag_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(device, "imgui-frag", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/imgui-frag.spv")))));

        let font_texture_create =
            vk::ImageCreateInfo::builder()
                .format(vk::Format::R8G8B8A8_UNORM)
                .image_type(vk::ImageType::TYPE_2D)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                .extent(vk::Extent3D::builder()
                    .height(font_atlas.height)
                    .width(font_atlas.width)
                    .depth(1)
                    .build())
                .mip_levels(1)
                .array_layers(1)
                .build();

        let mut font_texture = image::create_from_bytes(
            0 as u64,
            device,
            allocator,
            immediate_command_buffer,
            render_context.get_graphics_queue_index(),
            render_context.get_graphics_queue(),
            font_texture_create,
            font_atlas.data,
            "font-atlas"
        );

        let font_sampler = unsafe {
            let sampler_create = vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .build();

            let sampler = device.get().create_sampler(&sampler_create, None)
                .expect("Failed to create font texture sampler");
            device.set_debug_name(vk::ObjectType::SAMPLER, sampler.as_raw(), "font_sampler");
            sampler
        };

        font_texture.get_image_mut().sampler = Some(font_sampler);

        // ugly way to update the font_texture layout bookkeeping after the copy completes
        if let Some(resolved_font_texture) = font_texture.resource_type.as_mut() {
            if let ResourceType::Image(font_texture_image) = resolved_font_texture {
                font_texture_image.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            } else {
                panic!("Font texture somehow not an image");
            }
        } else {
            panic!("Font texture somehow not valid");
        }

        unsafe {
            // ensure we've waited for the font buffer -> image copy to be complete
            // so that we don't attempt to destroy the buffer while it's still in-use by
            // a command buffer
            device.get().device_wait_idle()
                .expect("Error while waiting for font buffer -> image copy operation to complete");
        }

        ImguiRender {
            vertex_shader: vert_shader,
            fragment_shader: frag_shader,
            font_texture: Arc::new(Mutex::new(font_texture)),
        }
    }

    pub fn generate_passes(
        &self,
        allocator: Arc<Mutex<ResourceAllocator>>,
        draw_data: &DrawData,
        render_target: AttachmentReference,
        device: &DeviceInterface) -> Vec<PassType> {

        enter_span!(tracing::Level::TRACE, "Generate Imgui Passes");

        let mut pass_nodes: Vec<PassType> = Vec::new();
        // one passnode per drawlist
        pass_nodes.reserve(draw_data.draw_lists_count());

        // display data (scale and pos) is shared for all draw lists
        let display_buffer = {
            let display_create_info = BufferCreateInfo::new(
                vk::BufferCreateInfo::builder()
                    .size(std::mem::size_of::<DisplayBuffer>() as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                    .build(),
                "Imgui_display_buffer".to_string());
            let display_buffer = device.create_buffer(
                0, // TODO: need to generate a real handle value
                &display_create_info,
                allocator.clone(),
                MemoryLocation::CpuToGpu);

            device.update_buffer(&display_buffer, |mapped_memory: *mut c_void, _size: u64| {
                let mut display_scale: [f32; 2] = [0.0, 0.0];
                display_scale[0] = 2.0 / draw_data.display_size[0];
                display_scale[1] = 2.0 / draw_data.display_size[1];

                let mut display_pos: [f32; 2] = [0.0, 0.0];
                display_pos[0] = -1.0 - draw_data.display_pos[0] * display_scale[0];
                display_pos[1] = -1.0 - draw_data.display_pos[1] * display_scale[1];

                let display_value = DisplayBuffer {
                    scale: display_scale,
                    pos: display_pos
                };

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        &display_value,
                        mapped_memory as *mut DisplayBuffer,
                        1);
                }
            });

            Rc::new(RefCell::new(display_buffer))
        };


        for draw_list in draw_data.draw_lists() {
            let vtx_create = BufferCreateInfo::new(vk::BufferCreateInfo::builder()
                                                       .size((draw_data.total_vtx_count as usize * std::mem::size_of::<DrawVert>()) as vk::DeviceSize)
                                                       .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                                                       .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                                       .build(),
                                                   "imgui_vtx_buffer".to_string());

            let vtx_buffer = Rc::new(RefCell::new(device.create_buffer(
                0, // TODO: create real buffer handle
                &vtx_create,
                allocator.clone(),
                MemoryLocation::CpuToGpu)));
            let vtx_data = draw_list.vtx_buffer();
            device.update_buffer(&vtx_buffer.borrow(), |mapped_memory: *mut c_void, _size: u64| {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        vtx_data.as_ptr(),
                        mapped_memory as *mut DrawVert,
                        vtx_data.len()
                    )
                }
            });

            let idx_create = BufferCreateInfo::new(vk::BufferCreateInfo::builder()
                                                       .size((draw_data.total_idx_count as usize * std::mem::size_of::<DrawIdx>()) as vk::DeviceSize)
                                                       .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                                                       .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                                       .build(),
                                                   "imgui_idx_buffer".to_string());

            let idx_buffer = Rc::new(RefCell::new(device.create_buffer(
                0, // TODO: create buffer handle
                &idx_create,
                allocator.clone(),
                MemoryLocation::CpuToGpu)));

            let idx_data = draw_list.idx_buffer();
            device.update_buffer(&idx_buffer.borrow(), |mapped_memory: *mut c_void, _size: u64| {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        idx_data.as_ptr(),
                        mapped_memory as *mut DrawIdx,
                        idx_data.len()
                    )
                }
            });

            let idx_length = idx_data.len() as u32;

            let font_binding = ResourceBinding {
                resource: self.font_texture.clone(),
                binding_info: BindingInfo {
                    binding_type: BindingType::Image(ImageBindingInfo{
                        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                    }),
                    set: 0,
                    slot: 1,
                    stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                    access: vk::AccessFlags::SHADER_READ
                }
            };

            let vertex_input = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_binding_descriptions(std::slice::from_ref(&IMGUI_VERTEX_BINDING))
                .vertex_attribute_descriptions(&IMGUI_VERTEX_ATTRIBUTES)
                .build();

            let dynamic_states = vec!(vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR);

            let pipeline_description = PipelineDescription::new(
                vertex_input,
                dynamic_states,
                RasterizationType::Standard,
                DepthStencilType::Disable,
                BlendType::Transparent,
                "imgui",
                self.vertex_shader.clone(),
                self.fragment_shader.clone());

            let display_binding = ResourceBinding {
                resource: display_buffer.clone(),
                binding_info: BindingInfo {
                    binding_type: BindingType::Buffer(BufferBindingInfo{
                        offset: 0,
                        range: std::mem::size_of::<DisplayBuffer>() as vk::DeviceSize }),
                    set: 0,
                    slot: 0,
                    stage: vk::PipelineStageFlags::VERTEX_SHADER,
                    access: vk::AccessFlags::SHADER_READ,
                },
            };

            let (viewport, scissor) = {
                let extent = render_target.resource_image.borrow().get_image().extent;
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

            let pass_node = GraphicsPassNode::builder("imgui".to_string())
                .pipeline_description(pipeline_description)
                .render_target(render_target.clone())
                .read(font_binding)
                .read(display_binding)
                .tag(idx_buffer.clone())
                .tag(vtx_buffer.clone())
                .viewport(viewport)
                .scissor(scissor)
                .fill_commands(Box::new(
                    move |render_ctx: &VulkanRenderContext,
                          command_buffer: &vk::CommandBuffer | {
                        unsafe {
                            enter_span!(tracing::Level::TRACE, "Imgui Draw");
                            // let x = render_ctx.get_device().borrow().get()
                            let device = render_ctx.get_device();
                            let borrowed_device = device.borrow();
                            enter_gpu_span!("Imgui Draw GPU", "UI", borrowed_device.get(), command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);
                            // set vertex buffer
                            {
                                if let ResourceType::Buffer(vb) = &vtx_buffer.borrow().resource_type.as_ref().unwrap() {
                                    render_ctx.get_device().borrow().get().cmd_bind_vertex_buffers(
                                        *command_buffer,
                                        0,
                                         &[vb.buffer],
                                        &[0 as vk::DeviceSize]
                                    );
                                } else {
                                    panic!("Invalid vertex buffer for Imgui draw");
                                }
                            }

                            // set index buffer
                            {
                                if let ResourceType::Buffer(ib) = &idx_buffer.borrow().resource_type.as_ref().unwrap() {
                                    render_ctx.get_device().borrow().get().cmd_bind_index_buffer(
                                        *command_buffer,
                                        ib.buffer,
                                        0 as vk::DeviceSize,
                                        vk::IndexType::UINT16
                                    );
                                } else {
                                    panic!("Invalid index buffer for Imgui draw");
                                }
                            }

                            render_ctx.get_device().borrow().get().cmd_draw_indexed(
                                *command_buffer,
                                idx_length,
                                1,
                                0,
                                0,
                                0);
                        }
                    }
                ))
                .build()
                .expect("Failed to create imgui passnode");

            pass_nodes.push(PassType::Graphics(pass_node));
        }

        pass_nodes
    }
}
