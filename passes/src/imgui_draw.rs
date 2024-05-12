use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use ash::vk;
use ash::vk::{DeviceSize, Handle};
use gpu_allocator::MemoryLocation;
use imgui::{DrawData, DrawVert, DrawIdx};

use context::api_types::image::ImageCreateInfo;
use context::api_types::buffer::BufferCreateInfo;
use context::api_types::device::{DeviceResource, DeviceWrapper, ResourceType};
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ImageBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::{BlendType, DepthStencilType, PipelineDescription, RasterizationType};
use framegraph::shader;
use framegraph::shader::Shader;

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

pub struct ImguiRender {
    vertex_shader: Rc<RefCell<Shader>>,
    fragment_shader: Rc<RefCell<Shader>>,
    font_texture: Rc<RefCell<DeviceResource>>
}

impl Drop for ImguiRender {
    fn drop(&mut self) {
        println!("Dropping ImguiRender");
    }
}

impl ImguiRender {
    pub fn new(
        device: Rc<RefCell<DeviceWrapper>>,
        render_context: &VulkanRenderContext,
        font_atlas: imgui::FontAtlasTexture) -> ImguiRender {

        let vert_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(device.clone(), "imgui-vert", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/imgui-vert.spv")))));
        let frag_shader = Rc::new(RefCell::new(
            shader::create_shader_module_from_bytes(device.clone(), "imgui-frag", include_bytes!(concat!(env!("OUT_DIR"), "/shaders/imgui-frag.spv")))));

        let font_buffer_create = BufferCreateInfo::new(
            vk::BufferCreateInfo::builder()
                .size(font_atlas.data.len() as DeviceSize)
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .build(),
            "font_copy_buffer".to_string());

        let font_buffer = DeviceWrapper::create_buffer(
            device.clone(),
            &font_buffer_create,
            MemoryLocation::CpuToGpu);

        device.borrow().update_buffer(&font_buffer, |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    font_atlas.data.as_ptr(),
                    mapped_memory as *mut u8,
                    font_atlas.data.len());
            }
        });

        let font_texture_create = ImageCreateInfo::new(
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
                .build(),
            "font_atlast_texture".to_string());

        let mut font_texture = DeviceWrapper::create_image(
            device.clone(),
            &font_texture_create,
            MemoryLocation::GpuOnly);

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

            let sampler = device.borrow().get().create_sampler(&sampler_create, None)
                .expect("Failed to create font texture sampler");
            device.borrow().set_debug_name(vk::ObjectType::SAMPLER, sampler.as_raw(), "font_sampler");
            sampler
        };

        font_texture.get_image_mut().sampler = Some(font_sampler);

        {
            let resolved_buffer = {
                let resolved_resource = font_buffer.resource_type.as_ref().expect("Invalid font buffer");
                match resolved_resource {
                    ResourceType::Buffer(buffer) => { buffer },
                    _ => { panic!("Non-buffer resource type for font buffer")}
                }
            };
            let resolved_texture = {
                let resolved_resource = font_texture.resource_type.as_ref().expect("Invalid font texture");
                match resolved_resource {
                    ResourceType::Image(image) => { image },
                    _ => { panic!("Non-image resource type for font texture")}
                }
            };

            let copy_region = vk::BufferImageCopy::builder()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .base_array_layer(0)
                    .mip_level(0)
                    .build())
                .image_offset(vk::Offset3D::builder()
                    .x(0)
                    .y(0)
                    .z(0)
                    .build())
                .image_extent(resolved_texture.extent.clone())
                .build();

            let barrier_subresource_range = vk::ImageSubresourceRange::builder()
                .level_count(1)
                .base_mip_level(0)
                .layer_count(1)
                .base_array_layer(0)
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .build();

            let pre_barrier = vk::ImageMemoryBarrier::builder()
                .image(resolved_texture.image)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .subresource_range(barrier_subresource_range.clone())
                .src_queue_family_index(render_context.get_graphics_queue_index())
                .dst_queue_family_index(render_context.get_graphics_queue_index())
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .build();

            let post_barrier = vk::ImageMemoryBarrier::builder()
                .image(resolved_texture.image)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .subresource_range(barrier_subresource_range.clone())
                .src_queue_family_index(render_context.get_graphics_queue_index())
                .dst_queue_family_index(render_context.get_graphics_queue_index())
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .build();

            unsafe {
                let cb = render_context.get_immediate_command_buffer();
                device.borrow().get().reset_command_buffer(
                    cb,
                    vk::CommandBufferResetFlags::empty())
                    .expect("Failed to reset command buffer");

                let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                    .build();
                device.borrow().get().begin_command_buffer(cb, &command_buffer_begin_info)
                    .expect("Failed to begin recording command buffer");

                device.borrow().get().cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    std::slice::from_ref(&pre_barrier));

                device.borrow().get().cmd_copy_buffer_to_image(
                    cb,
                    resolved_buffer.buffer,
                    resolved_texture.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    std::slice::from_ref(&copy_region));

                device.borrow().get().cmd_pipeline_barrier(
                    cb,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::VERTEX_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    std::slice::from_ref(&post_barrier));

                device.borrow().get().end_command_buffer(cb)
                    .expect("Failed to record command buffer");

                let submit = vk::SubmitInfo::builder()
                    .command_buffers(std::slice::from_ref(&cb))
                    .build();

                device.borrow().get().queue_submit(
                    render_context.get_graphics_queue(),
                    std::slice::from_ref(&submit),
                        vk::Fence::null())
                    .expect("Failed to execute buffer->image copy");
            }
        }

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
            device.borrow().get().device_wait_idle()
                .expect("Error while waiting for font buffer -> image copy operation to complete");
        }

        ImguiRender {
            vertex_shader: vert_shader,
            fragment_shader: frag_shader,
            font_texture: Rc::new(RefCell::new(font_texture)),
        }
    }

    pub fn generate_passes(
        &self,
        draw_data: &DrawData,
        render_target: AttachmentReference,
        device: Rc<RefCell<DeviceWrapper>>) -> Vec<PassType> {

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
            let display_buffer = DeviceWrapper::create_buffer(
                device.clone(),
                &display_create_info,
                MemoryLocation::CpuToGpu);

            device.borrow().update_buffer(&display_buffer, |mapped_memory: *mut c_void, _size: u64| {
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

            let vtx_buffer = Rc::new(RefCell::new(DeviceWrapper::create_buffer(
                device.clone(),
                &vtx_create,
                MemoryLocation::CpuToGpu)));
            let vtx_data = draw_list.vtx_buffer();
            device.borrow().update_buffer(&vtx_buffer.borrow(), |mapped_memory: *mut c_void, _size: u64| {
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

            let idx_buffer = Rc::new(RefCell::new(DeviceWrapper::create_buffer(
                device.clone(),
                &idx_create,
                MemoryLocation::CpuToGpu)));

            let idx_data = draw_list.idx_buffer();
            device.borrow().update_buffer(&idx_buffer.borrow(), |mapped_memory: *mut c_void, _size: u64| {
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
