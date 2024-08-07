use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use ash::vk;
use ash::vk::DeviceSize;
use gpu_allocator::MemoryLocation;
use image::{DynamicImage, GenericImageView, ImageReader};
use image::DynamicImage::*;
use api_types::buffer::BufferCreateInfo;
use api_types::device::{DeviceResource, DeviceWrapper, ResourceType};
use api_types::image::{ImageCreateInfo, ImageType};
use context::vulkan_render_context::VulkanRenderContext;

pub fn create_from_bytes(
    device: Rc<RefCell<DeviceWrapper>>,
    render_context: &VulkanRenderContext,
    image_info: vk::ImageCreateInfo,
    image_bytes: &[u8],
    name: &str) -> DeviceResource {
    // create CPU-to-GPU buffer
    let buffer_create = BufferCreateInfo::new(
        vk::BufferCreateInfo::builder()
            .size(image_bytes.len() as DeviceSize)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build(),
        name.to_string()
    );
    let buffer = DeviceWrapper::create_buffer(
        device.clone(),
        &buffer_create,
        MemoryLocation::CpuToGpu
    );

    // update buffer with image bytes
    device.borrow().update_buffer(&buffer, |mapped_memory: *mut c_void, _size: u64| {
        unsafe {
            core::ptr::copy_nonoverlapping(
                image_bytes.as_ptr(),
                mapped_memory as *mut u8,
                image_bytes.len()
            );
        }
    });

    // create image
    let image_create = ImageCreateInfo::new(
        image_info,
        name.to_string(),
        ImageType::Color
    );
    let image = DeviceWrapper::create_image(
        device.clone(),
        &image_create,
        MemoryLocation::GpuOnly // TODO: this should be parameterized
    );

    // perform BufferImageCopy
    {
        let resolved_buffer = {
            let resolved_resource = buffer.resource_type.as_ref().expect("Invalid image-copy buffer");
            match resolved_resource {
                ResourceType::Buffer(buffer) => { buffer },
                _ => { panic!("Non-buffer resource type for image-copy buffer")}
            }
        };
        let resolved_texture = {
            let resolved_resource = image.resource_type.as_ref().expect("Invalid image");
            match resolved_resource {
                ResourceType::Image(image) => { image },
                _ => { panic!("Non-image resource type for buffer-to-image copy")}
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

        image
    }
}
pub fn create_from_uri(
    device: Rc<RefCell<DeviceWrapper>>,
    render_context: &VulkanRenderContext,
    uri: &str,
    is_linear: bool
) -> DeviceResource {
    let mut img = {
        let image = ImageReader::open(uri)
            .expect("Unable to load image");
        image.decode()
            .expect("Unable to decode image")
    };

    let format = {
        match img {
            ImageRgb16(_) => { vk::Format::R16G16B16_SFLOAT}
            ImageRgba16(_) => { vk::Format::R16G16B16A16_SFLOAT}
            ImageRgb32F(_) => { vk::Format::R32G32B32_SFLOAT}
            ImageRgba32F(_) => { vk::Format::R32G32B32A32_SFLOAT}
            _ => {
                match img {
                    ImageRgb8(_) => {
                        // 24-bit RGB image formats are not supported on Metal, so we are
                        // just going to cheat and convert to RGBA
                        // let corrected_img = img.into_rgba8();
                        img = DynamicImage::ImageRgba8(img.to_rgba8());
                        if is_linear {vk::Format::R8G8B8A8_UNORM} else {vk::Format::R8G8B8A8_SRGB}
                    }
                    ImageRgba8(_) => {
                        if is_linear {vk::Format::R8G8B8A8_UNORM} else {vk::Format::R8G8B8A8_SRGB}
                    }
                    _ => {
                        panic!("Unsupported format of loaded image")
                    }
                }
            }
        }
    };

    let texture_create = vk::ImageCreateInfo::builder()
        .format(format)
        .image_type(vk::ImageType::TYPE_2D)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .samples(vk::SampleCountFlags::TYPE_1)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .extent(vk::Extent3D::builder()
            .height(img.height())
            .width(img.width())
            .depth(1)
            .build())
        .mip_levels(1)
        .array_layers(1)
        .build();

    create_from_bytes(device, render_context, texture_create, img.as_bytes(), uri)
}
