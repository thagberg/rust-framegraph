use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::DeviceSize;
use gpu_allocator::MemoryLocation;
use image::{DynamicImage, GenericImageView, ImageReader};
use image::DynamicImage::*;
use api_types::buffer::BufferCreateInfo;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::interface::DeviceInterface;
use api_types::device::queue::QueueFamilies;
use api_types::device::resource::{DeviceResource, ResourceType};
use api_types::image::{ImageCreateInfo, ImageType};
use context::vulkan_render_context::VulkanRenderContext;

pub fn create_from_bytes<'a, 'b, 'c>(
    image_handle: u64,
    device: &'a DeviceInterface,
    allocator: Arc<Mutex<ResourceAllocator>>,
    immediate_command_buffer: &vk::CommandBuffer,
    graphics_queue_index: u32,
    graphics_queue: vk::Queue,
    image_info: vk::ImageCreateInfo,
    image_bytes: &'b [u8],
    name: &str) -> DeviceResource<'a, 'c> {
    // create CPU-to-GPU buffer
    let buffer_create = BufferCreateInfo::new(
        vk::BufferCreateInfo::default()
            .size(image_bytes.len() as DeviceSize)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE),
        name.to_string()
    );
    let buffer = device.create_buffer(
        0,
        &buffer_create,
        allocator.clone(),
        MemoryLocation::CpuToGpu
    );

    // update buffer with image bytes
    device.update_buffer(&buffer, |mapped_memory: *mut c_void, _size: u64| {
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
    let image = device.create_image(
        image_handle,
        &image_create,
        allocator.clone(),
        MemoryLocation::GpuOnly
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

        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .layer_count(1)
                .base_array_layer(0)
                .mip_level(0))
            .image_offset(vk::Offset3D::default()
                .x(0)
                .y(0)
                .z(0))
            .image_extent(resolved_texture.extent.clone());

        let barrier_subresource_range = vk::ImageSubresourceRange::default()
            .level_count(1)
            .base_mip_level(0)
            .layer_count(1)
            .base_array_layer(0)
            .aspect_mask(vk::ImageAspectFlags::COLOR);

        let pre_barrier = vk::ImageMemoryBarrier::default()
            .image(resolved_texture.image)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .subresource_range(barrier_subresource_range.clone())
            .src_queue_family_index(graphics_queue_index)
            .dst_queue_family_index(graphics_queue_index)
            .src_access_mask(vk::AccessFlags::NONE)
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

        let post_barrier = vk::ImageMemoryBarrier::default()
            .image(resolved_texture.image)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .subresource_range(barrier_subresource_range.clone())
            .src_queue_family_index(graphics_queue_index)
            .dst_queue_family_index(graphics_queue_index)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);

        unsafe {
            device.get().reset_command_buffer(
                *immediate_command_buffer,
                vk::CommandBufferResetFlags::empty())
                .expect("Failed to reset command buffer");

            let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            device.get().begin_command_buffer(*immediate_command_buffer, &command_buffer_begin_info)
                .expect("Failed to begin recording command buffer");

            device.get().cmd_pipeline_barrier(
                *immediate_command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&pre_barrier));

            device.get().cmd_copy_buffer_to_image(
                *immediate_command_buffer,
                resolved_buffer.buffer,
                resolved_texture.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&copy_region));

            device.get().cmd_pipeline_barrier(
                *immediate_command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::VERTEX_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&post_barrier));

            device.get().end_command_buffer(*immediate_command_buffer)
                .expect("Failed to record command buffer");

            let submit = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(immediate_command_buffer));

            device.get().queue_submit(
                graphics_queue,
                std::slice::from_ref(&submit),
                vk::Fence::null())
                .expect("Failed to execute buffer->image copy");

            // TODO: this is very bad and we should figure something else out
            device.get().device_wait_idle()
                .expect("Error when waiting for buffer->image copy");
        }

        image
    }
}
pub fn create_from_uri<'a, 'b>(
    image_handle: u64,
    device: &'a DeviceInterface,
    allocator: Arc<Mutex<ResourceAllocator>>,
    immediate_command_buffer: &vk::CommandBuffer,
    graphics_queue_index: u32,
    graphics_queue: vk::Queue,
    uri: &str,
    is_linear: bool
) -> DeviceResource<'a, 'b> {
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

    let texture_create = vk::ImageCreateInfo::default()
        .format(format)
        .image_type(vk::ImageType::TYPE_2D)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .samples(vk::SampleCountFlags::TYPE_1)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .extent(vk::Extent3D::default()
            .height(img.height())
            .width(img.width())
            .depth(1))
        .mip_levels(1)
        .array_layers(1);

    create_from_bytes(
        image_handle,
        device,
        allocator,
        immediate_command_buffer,
        graphics_queue_index,
        graphics_queue,
        texture_create,
        img.as_bytes(),
        uri)
}

pub fn get_aspect_mask_from_format(format: vk::Format) -> vk::ImageAspectFlags {
    match format {
        vk::Format::D16_UNORM |
        vk::Format::D32_SFLOAT => {
            vk::ImageAspectFlags::DEPTH
        },
        vk::Format::D16_UNORM_S8_UINT |
        vk::Format::D24_UNORM_S8_UINT |
        vk::Format::D32_SFLOAT_S8_UINT => {
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
        },
        vk::Format::S8_UINT => {
            vk::ImageAspectFlags::STENCIL
        },
        vk::Format::UNDEFINED => {
            panic!("Can't get aspect mask from undefined image format")
        },
        _ => {
            vk::ImageAspectFlags::COLOR
        }
    }
}
