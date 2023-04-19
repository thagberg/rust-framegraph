use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use ash::vk;
use ash::vk::{DeviceSize, Handle, wl_display};
use gpu_allocator::MemoryLocation;
use imgui::{DrawData, DrawVert, DrawIdx};

use context::api_types::image::ImageCreateInfo;
use context::api_types::buffer::BufferCreateInfo;
use context::api_types::device::{DeviceResource, DeviceWrapper, ResourceType};
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, BindingType, ImageBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;

pub struct ImguiRender {
    font_texture: Rc<RefCell<DeviceResource>>
}

impl ImguiRender {
    pub fn new(
        device: Rc<RefCell<DeviceWrapper>>,
        render_context: &VulkanRenderContext,
        font_atlas: imgui::FontAtlasTexture) -> ImguiRender {

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

        device.borrow().update_buffer(&font_buffer, |mapped_memory: *mut c_void, size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    font_atlas.data.as_ptr(),
                    mapped_memory as *mut u8,
                    font_atlas.data.len());
            }
        });

        let font_texture_create = ImageCreateInfo::new(
            vk::ImageCreateInfo::builder()
                .format(vk::Format::R8G8B8A8_SRGB)
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

        ImguiRender {
            font_texture: Rc::new(RefCell::new(font_texture)),
        }
    }

    pub fn generate_passes(
        &self,
        draw_data: &DrawData,
        render_target: Rc<RefCell<DeviceResource>>,
        device: Rc<RefCell<DeviceWrapper>>) -> Vec<GraphicsPassNode> {

        let mut pass_nodes: Vec<GraphicsPassNode> = Vec::new();
        // one passnode per drawlist
        pass_nodes.reserve(draw_data.draw_lists_count());

        for draw_list in draw_data.draw_lists() {
            let vtx_create = BufferCreateInfo::new(vk::BufferCreateInfo::builder()
                                                       .size((draw_data.total_vtx_count as usize * std::mem::size_of::<DrawVert>()) as vk::DeviceSize)
                                                       .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                                                       .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                                       .build(),
                                                   "imgui_vtx_buffer".to_string());

            let vtx_buffer = DeviceWrapper::create_buffer(
                device.clone(),
                &vtx_create,
                MemoryLocation::CpuToGpu);
            let vtx_data = draw_list.vtx_buffer();
            device.borrow().update_buffer(&vtx_buffer, |mapped_memory: *mut c_void, size: u64| {
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

            let idx_buffer = DeviceWrapper::create_buffer(
                device.clone(),
                &idx_create,
                MemoryLocation::CpuToGpu);

            let idx_data = draw_list.idx_buffer();
            device.borrow().update_buffer(&idx_buffer, |mapped_memory: *mut c_void, size: u64| {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        idx_data.as_ptr(),
                        mapped_memory as *mut DrawIdx,
                        idx_data.len()
                    )
                }
            });

            let vtx_length = vtx_data.len() as u32;

            let rt_ref = AttachmentReference::new(
                render_target.clone(),
                vk::Format::R8G8B8A8_SRGB, // TODO: this should be parameterized
                vk::SampleCountFlags::TYPE_1,
                vk::AttachmentLoadOp::LOAD,
                vk::AttachmentStoreOp::STORE);

            let font_binding = ResourceBinding {
                resource: self.font_texture.clone(),
                binding_info: BindingInfo {
                    binding_type: BindingType::Image(ImageBindingInfo{
                        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                    }),
                    set: 0,
                    slot: 0,
                    stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                    access: vk::AccessFlags::SHADER_READ
                }
            };

            let pass_node = GraphicsPassNode::builder("imgui".to_string())
                .render_target(rt_ref)
                .read(font_binding)
                .fill_commands(Box::new(
                    move |render_ctx: &VulkanRenderContext,
                          command_buffer: &vk::CommandBuffer | {
                        println!("Rendering Imgui drawlists");

                        unsafe {
                            render_ctx.get_device().borrow().get().cmd_draw(
                                *command_buffer,
                                vtx_length,
                                1,
                                0,
                                0);
                        }
                    }
                ))
                .build()
                .expect("Failed to create imgui passnode");

            pass_nodes.push(pass_node);
        }

        pass_nodes
    }
}
