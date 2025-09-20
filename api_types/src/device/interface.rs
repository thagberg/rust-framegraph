use std::ffi::{c_void, CString};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use ash::vk;
use ash::vk::{DebugUtilsLabelEXT, DebugUtilsObjectNameInfoEXT, Handle, ObjectType};
use gpu_allocator::MemoryLocation;
use crate::buffer::{BufferCreateInfo, BufferWrapper};
use crate::device::debug::VulkanDebug;
use crate::device::allocator::ResourceAllocator;
use crate::device::queue::QueueFamilies;
use crate::device::resource::{DeviceResource, ResourceType};
use crate::image::{ImageCreateInfo, ImageType, ImageWrapper};
use crate::pipeline::DevicePipeline;
use crate::renderpass::DeviceRenderpass;
use crate::shader::DeviceShader;

pub struct DeviceInterface {
    device: ash::Device,
    queue_families: QueueFamilies,
    debug: Option<VulkanDebug>
}

impl Debug for DeviceInterface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceInterface")
            .finish()
    }
}

impl Drop for DeviceInterface {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

impl Deref for DeviceInterface {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl DeviceInterface {
    pub fn new(
        device: ash::Device,
        queue_families: QueueFamilies,
        debug: Option<VulkanDebug>) -> Self {
        DeviceInterface {
            device,
            queue_families,
            debug
        }
    }

    pub fn get(&self) -> &ash::Device { &self.device }

    pub fn get_queue_families(&self) -> &QueueFamilies { &self.queue_families }

    pub fn set_debug_name<T: ash::vk::Handle>(&self, handle: T, name: &str)
    {
        let c_name = CString::new(name)
            .expect("Failed to create C-name for debug object");
        let debug_info = DebugUtilsObjectNameInfoEXT::default()
            .object_handle(handle)
            .object_name(&c_name);
        if let Some(debug) = &self.debug {
            debug.set_object_name(&debug_info);
        }
    }

    pub fn set_image_name(&self, image: &ImageWrapper, name: &str)
    {
        self.set_debug_name(image.get(), name);
    }

    pub fn create_image_view(
        &self,
        image: vk::Image,
        format: vk::Format,
        image_view_flags: vk::ImageViewCreateFlags,
        aspect_flags: vk::ImageAspectFlags,
        mip_levels: u32) -> vk::ImageView
    {
        let create_info = vk::ImageViewCreateInfo::default()
            .flags(image_view_flags)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY
            })
            .subresource_range( vk::ImageSubresourceRange {
                aspect_mask: aspect_flags,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1
            })
            .image(image);

        unsafe {
            self.device.create_image_view(&create_info, None)
                .expect("Failed to create image view.")
        }
    }

    pub fn create_image(
        &self,
        handle: u64,
        image_desc: &ImageCreateInfo,
        allocator: Arc<Mutex<ResourceAllocator>>,
        memory_location: MemoryLocation) -> DeviceResource {
        let device_image = {
            let create_info = image_desc.get_create_info();
            let image = unsafe {
                self.device.create_image(create_info, None)
                    .expect("Failed to create image")
            };

            let memory_requirements = unsafe {
                self.device.get_image_memory_requirements(image)
            };

            let allocation = {
                let mut allocator_ref = allocator.lock().unwrap();
                allocator_ref.allocate_memory(
                    image_desc.get_name(),
                    memory_requirements,
                    memory_location,
                    false)
            };

            unsafe {
                self.device.bind_image_memory(
                    image,
                    allocation.memory(),
                    allocation.offset())
                    .expect("Failed to bind image to memory");
            }

            let aspect_flags = match image_desc.get_image_type() {
                ImageType::Color => {
                    vk::ImageAspectFlags::COLOR
                }
                ImageType::Depth => {
                    vk::ImageAspectFlags::DEPTH
                }
                ImageType::DepthStencil => {
                    vk::ImageAspectFlags::STENCIL | vk::ImageAspectFlags::DEPTH
                }
                ImageType::Stencil => {
                    vk::ImageAspectFlags::STENCIL
                }
            };

            let image_view = self.create_image_view(
                image,
                // vk::Format::R8G8B8A8_SRGB,
                image_desc.get_create_info().format,
                vk::ImageViewCreateFlags::empty(),
                aspect_flags,
                1);
            self.set_debug_name(image_view, image_desc.get_name());
            let image_wrapper = ImageWrapper::new(
                image,
                image_view,
                create_info.initial_layout,
                create_info.extent,
                false, // Swapchain images only go through wrap_image
                create_info.format,
                None);

            self.set_image_name(&image_wrapper, image_desc.get_name());
            DeviceResource::new(
                Some(allocation),
                Some(ResourceType::Image(image_wrapper)),
                handle,
                &self,
                Some(allocator.clone())
            )
        };

        device_image
    }

    pub fn destroy_image(&self, image: &ImageWrapper) {
        unsafe {
            if let Some(sampler) = image.sampler {
                self.device.destroy_sampler(sampler, None);
            }
            self.device.destroy_image_view(image.view, None);
            // We're not responsible for cleaning up the swapchain images
            if !image.is_swapchain_image {
                self.device.destroy_image(image.image, None);
            }
        }
    }

    pub fn wrap_image(
        &self,
        handle: u64,
        image: vk::Image,
        format: vk::Format,
        image_aspect_flags: vk::ImageAspectFlags,
        mip_levels: u32,
        extent: vk::Extent3D,
        is_swapchain_image: bool
    ) -> DeviceResource {
        let image_view = self.create_image_view(
            image,
            format,
            vk::ImageViewCreateFlags::empty(),
            image_aspect_flags,
            mip_levels);

        let image_wrapper = ImageWrapper::new(
            image,
            image_view,
            vk::ImageLayout::UNDEFINED,
            extent,
            is_swapchain_image,
            format,
            None);

        DeviceResource::new(
            None,
            Some(ResourceType::Image(image_wrapper)),
            handle,
            &self,
            None
        )
    }

    pub fn set_buffer_name(&self, buffer: &BufferWrapper, name: &str)
    {
        self.set_debug_name(buffer.get(), name);
    }

    pub fn create_buffer<'m>(
        &self,
        handle: u64,
        buffer_desc: &'m BufferCreateInfo<'m>,
        allocator: Arc<Mutex<ResourceAllocator>>,
        memory_location: MemoryLocation) -> DeviceResource<'_, 'm> {

        let device_buffer = {
            log::trace!(target: "resource", "Creating buffer: {} -- {}", handle, buffer_desc.get_name());

            let create_info = buffer_desc.get_create_info();
            let buffer = unsafe {
                self.device.create_buffer(create_info, None)
                    .expect("Failed to create buffer")
            };

            let memory_requirements = unsafe {
                self.device.get_buffer_memory_requirements(buffer)
            };

            let allocation = {
                let mut allocator_ref = allocator.lock().unwrap();
                allocator_ref.allocate_memory(
                    buffer_desc.get_name(),
                    memory_requirements,
                    memory_location,
                    true)
            };

            unsafe {
                self.device.bind_buffer_memory(
                    buffer,
                    allocation.memory(),
                    allocation.offset())
                    .expect("Failed to bind buffer to memory");
            }

            let buffer_wrapper = BufferWrapper::new(buffer, buffer_desc.get_create_info().clone());
            self.set_buffer_name(&buffer_wrapper, buffer_desc.get_name());
            DeviceResource::new(
                Some(allocation),
                Some(ResourceType::Buffer(buffer_wrapper)),
                handle,
                &self,
                Some(allocator.clone())
            )
        };
        device_buffer
    }

    pub fn update_buffer<F>(
        &self,
        device_buffer: &DeviceResource,
        mut fill_callback: F)
    where F: FnMut(*mut c_void, u64) {

        log::trace!(target: "resource", "Updating buffer: {}", device_buffer.get_handle());

        let allocation = device_buffer.allocation.as_ref()
            .expect("Cannot update buffer with no allocation");
        if let Some(resolved_resource) = &device_buffer.resource_type {
            if let ResourceType::Buffer(resolved_buffer) = &resolved_resource {
                let mapped_range = unsafe {
                    vk::MappedMemoryRange::default()
                        .memory(allocation.memory())
                        .size(vk::WHOLE_SIZE)
                        .offset(allocation.offset())
                };
                if let Some(mapped) = allocation.mapped_ptr() {
                    // TODO: I believe this will occur if the memory is already host-visible?
                    fill_callback(mapped.as_ptr(), allocation.size());
                } else {
                    unsafe {
                        let mapped_memory = self.device.map_memory(
                            allocation.memory(),
                            allocation.offset(),
                            allocation.size(),
                            vk::MemoryMapFlags::empty())
                            .expect("Failed to map buffer");
                        fill_callback(mapped_memory, allocation.size());
                        self.device.unmap_memory(allocation.memory());
                    }
                }
                // If this buffer was not allocated in host-coherent memory then we need
                // to flush the mapped ranges before the changes will be visible.
                // This shouldn't be done on host-coherent memory because coherent memory
                // allocations might not be aligned with vkPhysicalDeviceLimits::nonCoherentAtomSize,
                // which is required for vkFlushMappedMemoryRanges
                if !allocation.memory_properties().contains(vk::MemoryPropertyFlags::HOST_COHERENT) {
                    unsafe {
                        self.device.flush_mapped_memory_ranges(std::slice::from_ref(&mapped_range))
                            .expect("Failed to flush mapped memory");
                    }
                }
            } else {
                panic!("Cannot update a non-buffer resource as a buffer");
            }
        } else {
            panic!("Cannot update an invalid buffer");
        }
    }

    pub fn destroy_buffer(&self, buffer: &BufferWrapper) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
        }
    }

    pub fn create_shader(
        &self,
        name: &str,
        shader_create: &vk::ShaderModuleCreateInfo) -> DeviceShader {

        let shader = unsafe {
            self.device.create_shader_module(&shader_create, None)
                .expect("Failed to create shader module")
        };

        self.set_debug_name(shader, name);

        DeviceShader::new(shader, &self)
    }

    pub fn create_pipeline(
        &self,
        create_info: &vk::GraphicsPipelineCreateInfo,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        name: &str
    ) -> DevicePipeline {
        let pipeline = unsafe {
            self.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(create_info),
                None)
                .expect("Failed to create Graphics Pipeline")
        }[0];

        self.set_debug_name(pipeline, name);
        self.set_debug_name(pipeline_layout, &(name.to_owned() + "_layout"));

        DevicePipeline::new(
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            &self)
    }

    pub fn create_compute_pipeline(
        &self,
        create_info: &vk::ComputePipelineCreateInfo,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        name: &str
    ) -> DevicePipeline {
        let pipeline = unsafe {
            self.device.create_compute_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(create_info),
                None)
                .expect("Failed to create Graphics Pipeline")
        }[0];

        self.set_debug_name(pipeline, name);
        self.set_debug_name(pipeline_layout, &(name.to_owned() + "_layout"));

        DevicePipeline::new(
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            &self)
    }

    pub fn create_renderpass(
        &self,
        create_info: &vk::RenderPassCreateInfo,
        name: &str
    ) -> DeviceRenderpass {
        let renderpass = unsafe {
            self.device.create_render_pass(create_info, None)
                .expect("Failed to create renderpass")
        };
        self.set_debug_name(renderpass, name);

        DeviceRenderpass::new(
            renderpass,
            &self
        )
    }

    pub fn push_debug_label(
        &self,
        command_buffer: vk::CommandBuffer,
        label: &str) {
        if let Some(debug) = &self.debug {
            debug.begin_label(label, command_buffer);
        }
    }

    pub fn pop_debug_label(
        &self,
        command_buffer: vk::CommandBuffer) {
        if let Some(debug) = &self.debug {
            debug.end_label(command_buffer);
        }
    }
}