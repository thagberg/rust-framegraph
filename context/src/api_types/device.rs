use std::cell::RefCell;
use std::ffi::{CString};
use core::ffi::c_void;
use std::rc::Rc;
use ash::{Device, vk};
use ash::extensions::ext::DebugUtils;
use ash::vk::{DebugUtilsLabelEXT, DebugUtilsObjectNameInfoEXT, Handle};
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;

use crate::api_types::image::{ImageWrapper, ImageCreateInfo};
use crate::api_types::buffer::{BufferWrapper, BufferCreateInfo};

#[derive(Copy, Clone)]
pub struct QueueFamilies {
    pub graphics: Option<u32>,
    pub compute: Option<u32>,
    pub present: Option<u32>
}

impl QueueFamilies {
    pub fn is_complete(&self) -> bool {
        self.graphics.is_some() && self.compute.is_some() && self.present.is_some()
    }
}

#[derive(Copy, Clone)]
pub struct PhysicalDeviceWrapper {
    physical_device: vk::PhysicalDevice,
}

impl PhysicalDeviceWrapper {
    pub fn new(physical_device: vk::PhysicalDevice) -> PhysicalDeviceWrapper {
        PhysicalDeviceWrapper {
            physical_device
        }
    }

    pub fn get(&self) -> vk::PhysicalDevice { self.physical_device }
}

/// DeviceLifetime exists to ensure DeviceWrapper can destroy its Allocator before
/// ash::Device::destroy_device gets called
pub struct DeviceLifetime {
    device: ash::Device
}

impl Drop for DeviceLifetime {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

impl DeviceLifetime {
    pub fn new(device: ash::Device) -> Self {
        DeviceLifetime {
            device
        }
    }

    pub fn get(&self) -> &ash::Device {
        &self.device
    }
}

pub struct DeviceWrapper {
    handle_generator: u64,
    debug_utils: DebugUtils,
    queue_family_indices: QueueFamilies,
    allocator: Allocator,
    device: DeviceLifetime,
}

impl Drop for DeviceWrapper {
    fn drop(&mut self) {
        unsafe {
            self.allocator.report_memory_leaks(log::Level::Warn);
        }
    }
}


#[derive(Clone)]
pub enum ResourceType {
    Buffer(BufferWrapper),
    Image(ImageWrapper)
}

pub struct DeviceResource {
    pub allocation: Option<Allocation>,
    pub resource_type: Option<ResourceType>,

    handle: u64,
    device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DeviceResource {
    fn drop(&mut self) {
        if let Some(resource_type) = &mut self.resource_type {
            match resource_type {
                ResourceType::Buffer(buffer) => {
                    self.device.borrow_mut().destroy_buffer(buffer);
                },
                ResourceType::Image(image) => {
                    self.device.borrow_mut().destroy_image(image);
                }
            }
        }
        if let Some(alloc) = &mut self.allocation {
            let moved = std::mem::replace(alloc, Allocation::default());
            self.device.borrow_mut().free_allocation(moved);
        }
    }
}

impl PartialEq<Self> for DeviceResource {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl Eq for DeviceResource {}

impl DeviceResource {
    pub fn get_image(&self) -> &ImageWrapper {
        match &self.resource_type {
            Some(resolved_resource) => {
                match &resolved_resource {
                    ResourceType::Image(image) => {
                        image
                    },
                    _ => {
                        panic!("Non-image resource type")
                    }
                }
            },
            None => {
                panic!("Unresolved resource")
            }
        }
    }

    pub fn get_image_mut(&mut self) -> &mut ImageWrapper {
        match self.resource_type.as_mut() {
            Some(resolved_resource) => {
                match resolved_resource {
                    ResourceType::Image(image) => {
                        image
                    },
                    _ => {
                        panic!("Non-image resource type")
                    }
                }
            },
            None => {
                panic!("Unresolved resource")
            }
        }
    }

    pub fn get_buffer(&self) -> &BufferWrapper {
        match &self.resource_type {
            Some(resolved_resource) => {
                match &resolved_resource {
                    ResourceType::Buffer(buffer) => {
                       buffer
                    },
                    _ => {
                        panic!("Non-buffer resource type")
                    }
                }
            },
            None => {
                panic!("Unresolved resource")
            }
        }
    }

    pub fn get_handle(&self) -> u64 {
        self.handle
    }
}

// pub struct DeviceDescriptorSet {
//     descriptor_set: vk::DescriptorSet,
//     descriptor_pool: vk::DescriptorPool,
//     device: Rc<RefCell<DeviceWrapper>>
// }
//
// impl Drop for DeviceDescriptorSet {
//     fn drop(&mut self) {
//         unsafe {
//             self.device.borrow().get().free_descriptor_sets(
//                 self.descriptor_pool,
//                 std::slice::from_ref(&self.descriptor_set))
//                 .expect("Failed to free descriptor set")
//         }
//     }
// }

pub struct DeviceFramebuffer {
    framebuffer: vk::Framebuffer,
    device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DeviceFramebuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.borrow().get().destroy_framebuffer(
                self.framebuffer,
                None);
        }
    }
}

impl DeviceFramebuffer {
    pub fn new(framebuffer: vk::Framebuffer, device: Rc<RefCell<DeviceWrapper>>) -> Self {
        DeviceFramebuffer {
            framebuffer: framebuffer,
            device: device
        }
    }

    pub fn get_framebuffer(&self) -> vk::Framebuffer { self.framebuffer }
}

impl DeviceWrapper {
    pub fn new(
        device: ash::Device,
        instance: &ash::Instance,
        physical_device: &PhysicalDeviceWrapper,
        debug_utils: DebugUtils,
        queue_family_indices: QueueFamilies) -> DeviceWrapper {

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false // https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPhysicalDeviceBufferDeviceAddressFeaturesEXT.html
        }).expect("Failed to create GPU memory allocator");

        DeviceWrapper {
            device: DeviceLifetime::new(device),
            debug_utils,
            queue_family_indices,
            allocator,
            handle_generator: 0
        }
    }
    pub fn get(&self) -> &ash::Device {
        self.device.get()
    }
    pub fn get_queue_family_indices(&self) -> &QueueFamilies { &self.queue_family_indices }

    pub fn get_debug_utils(&self) -> &DebugUtils { &self.debug_utils }

    pub fn free_allocation(&mut self, allocation: Allocation) {
        self.allocator.free(allocation)
            .expect("Failed to free Device allocation");
    }

    pub fn destroy_buffer(&mut self, buffer: &BufferWrapper) {
        unsafe {
            self.device.get().destroy_buffer(buffer.buffer, None);
        }
    }

    pub fn destroy_image(&mut self, image: &ImageWrapper) {
        unsafe {
            if let Some(sampler) = image.sampler {
                self.device.get().destroy_sampler(sampler, None);
            }
            self.device.get().destroy_image_view(image.view, None);
            // We're not responsible for cleaning up the swapchain images
            if !image.is_swapchain_image {
                self.device.get().destroy_image(image.image, None);
            }
        }
    }

    pub fn create_image_view(
        &self,
        image: vk::Image,
        format: vk::Format,
        image_view_flags: vk::ImageViewCreateFlags,
        aspect_flags: vk::ImageAspectFlags,
        mip_levels: u32) -> vk::ImageView
    {
        let create_info = vk::ImageViewCreateInfo {
            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: image_view_flags,
            view_type: vk::ImageViewType::TYPE_2D,
            format,
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY
            },
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: aspect_flags,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1
            },
            image: image
        };

        unsafe {
            self.device.get().create_image_view(&create_info, None)
                .expect("Failed to create image view.")
        }
    }

    pub fn set_debug_name(&self, object_type: vk::ObjectType, handle: u64, name: &str)
    {
        let c_name = CString::new(name)
            .expect("Failed to create C-name for debug object");
        let debug_info = DebugUtilsObjectNameInfoEXT::builder()
            .object_type(object_type)
            .object_handle(handle)
            .object_name(&c_name)
            .build();
        unsafe {
            self.debug_utils.debug_utils_set_object_name(self.device.get().handle(), &debug_info)
                .expect("Failed to set debug object name");
        }
    }

    pub fn set_image_name(&self, image: &ImageWrapper, name: &str)
    {
        self.set_debug_name(vk::ObjectType::IMAGE, image.get().as_raw(), name);
    }

    pub fn allocate_memory(
        &mut self,
        name: &str,
        requirements: vk::MemoryRequirements,
        location: MemoryLocation,
        linear: bool) -> Allocation {

        let alloc_name = name.to_owned() + "_allocation";
        self.allocator.allocate(&AllocationCreateDesc {
            name: &alloc_name,
            requirements,
            location,
            linear
        }).expect("Failed to allocate memory for Device resource")
    }

    pub fn generate_handle(
        &mut self
    ) -> u64 {
        let new = self.handle_generator;
        self.handle_generator += 1;
        new
    }

    // pub fn upload_image_contents(&self, image: Rc<RefCell<DeviceResource>>, data: &[u8]) {
    //
    // }

    pub fn create_image(
        device: Rc<RefCell<DeviceWrapper>>,
        image_desc: &ImageCreateInfo,
        memory_location: MemoryLocation) -> DeviceResource {

        let device_image = {
            let new_handle = device.borrow_mut().generate_handle();
            let create_info = image_desc.get_create_info();
            let image = unsafe {
                device.borrow().get().create_image(create_info, None)
                    .expect("Failed to create image")
            };

            let memory_requirements = unsafe {
                device.borrow().get().get_image_memory_requirements(image)
            };

            let allocation = device.borrow_mut().allocate_memory(
                image_desc.get_name(),
                memory_requirements,
                memory_location,
                false);

            unsafe {
                device.borrow().get().bind_image_memory(
                    image,
                    allocation.memory(),
                    allocation.offset())
                    .expect("Failed to bind image to memory");
            }

            let image_view = device.borrow().create_image_view(
                image,
                // vk::Format::R8G8B8A8_SRGB,
                image_desc.get_create_info().format,
                vk::ImageViewCreateFlags::empty(),
                vk::ImageAspectFlags::COLOR,
                1);
            device.borrow().set_debug_name(vk::ObjectType::IMAGE_VIEW, image_view.as_raw(), image_desc.get_name());
            let image_wrapper = ImageWrapper::new(
                image,
                image_view,
                create_info.initial_layout,
                create_info.extent,
                false, // Swapchain images only go through wrap_image
                create_info.format,
                None);

            device.borrow().set_image_name(&image_wrapper, image_desc.get_name());
            DeviceResource {
                allocation: Some(allocation),
                resource_type: Some(ResourceType::Image(image_wrapper)),
                handle: new_handle,
                device,
            }
        };

        device_image
    }

    pub fn wrap_image(
        device: Rc<RefCell<DeviceWrapper>>,
        image: vk::Image,
        format: vk::Format,
        image_aspect_flags: vk::ImageAspectFlags,
        mip_levels: u32,
        extent: vk::Extent3D,
        is_swapchain_image: bool
    ) -> DeviceResource {
        let new_handle = device.borrow_mut().generate_handle();

        let image_view = device.borrow().create_image_view(
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

        DeviceResource {
            allocation: None,
            resource_type: Some(ResourceType::Image(image_wrapper)),
            handle: new_handle,
            device
        }
    }

    pub fn set_buffer_name(&self, buffer: &BufferWrapper, name: &str)
    {
        self.set_debug_name(vk::ObjectType::BUFFER, buffer.get().as_raw(), name);
    }

    pub fn create_buffer(
        device: Rc<RefCell<DeviceWrapper>>,
        buffer_desc: &BufferCreateInfo,
        memory_location: MemoryLocation) -> DeviceResource {

        let device_buffer = {
            let new_handle = device.borrow_mut().generate_handle();
            let create_info = buffer_desc.get_create_info();
            let buffer = unsafe {
                device.borrow().get().create_buffer(create_info, None)
                    .expect("Failed to create buffer")
            };

            let memory_requirements = unsafe {
                device.borrow().get().get_buffer_memory_requirements(buffer)
            };

            let allocation = device.borrow_mut().allocate_memory(
                buffer_desc.get_name(),
                memory_requirements,
                memory_location,
                true);

            unsafe {
                device.borrow().get().bind_buffer_memory(
                    buffer,
                    allocation.memory(),
                    allocation.offset())
                    .expect("Failed to bind buffer to memory");
            }

            let buffer_wrapper = BufferWrapper::new(buffer);
            device.borrow().set_buffer_name(&buffer_wrapper, buffer_desc.get_name());
            DeviceResource {
                allocation: Some(allocation),
                resource_type: Some(ResourceType::Buffer(buffer_wrapper)),
                handle: new_handle,
                device
            }
        };
        device_buffer
    }

    pub fn update_buffer<F>(&self, device_buffer: &DeviceResource, mut fill_callback: F)
        where F: FnMut(*mut c_void, u64) {
        let allocation = {
            match &device_buffer.allocation {
                Some(alloc) => { alloc },
                _ => {
                    panic!("Cannot update buffer with no allocation");
                }
            }
        };
        if let Some(resolved_resource) = &device_buffer.resource_type {
            if let ResourceType::Buffer(_) = &resolved_resource {
                if let Some(mapped) = allocation.mapped_ptr() {
                    // TODO: I believe this will occur if the memory is already host-visible?
                    fill_callback(mapped.as_ptr(), allocation.size());
                } else {
                    unsafe {
                        let mapped_memory = self.device.get().map_memory(
                            allocation.memory(),
                            allocation.offset(),
                            allocation.size(),
                            vk::MemoryMapFlags::empty())
                            .expect("Failed to map buffer");
                        fill_callback(mapped_memory, allocation.size());
                        self.device.get().unmap_memory(allocation.memory());
                    }
                }
            } else {
                panic!("Cannot update a non-buffer resource as a buffer");
            }
        } else {
            panic!("Cannot update an invalid buffer");
        }
    }

    pub fn create_shader(
        device: Rc<RefCell<DeviceWrapper>>,
        shader_create: &vk::ShaderModuleCreateInfo) -> DeviceShader {

        let shader = unsafe {
            device.borrow().get().create_shader_module(&shader_create, None)
                .expect("Failed to create shader module")
        };

        DeviceShader::new(shader, device)
    }

    pub fn create_pipeline(
        device: Rc<RefCell<DeviceWrapper>>,
        create_info: &vk::GraphicsPipelineCreateInfo,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        name: &str
    ) -> DevicePipeline {
        let pipeline = unsafe {
            device.borrow().get().create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(create_info),
                None)
                .expect("Failed to create Graphics Pipeline")
        }[0];

        device.borrow().set_debug_name(vk::ObjectType::PIPELINE, pipeline.as_raw(), name);
        device.borrow().set_debug_name(vk::ObjectType::PIPELINE_LAYOUT, pipeline_layout.as_raw(), &(name.to_owned() + "_layout"));

        DevicePipeline::new(
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            device)
    }

    pub fn create_compute_pipeline(
        device: Rc<RefCell<DeviceWrapper>>,
        create_info: &vk::ComputePipelineCreateInfo,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        name: &str
    ) -> DevicePipeline {
        let pipeline = unsafe {
            device.borrow().get().create_compute_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(create_info),
                None)
                .expect("Failed to create Graphics Pipeline")
        }[0];

        device.borrow().set_debug_name(vk::ObjectType::PIPELINE, pipeline.as_raw(), name);
        device.borrow().set_debug_name(vk::ObjectType::PIPELINE_LAYOUT, pipeline_layout.as_raw(), &(name.to_owned() + "_layout"));

        DevicePipeline::new(
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            device)
    }

    pub fn create_renderpass(
        device: Rc<RefCell<DeviceWrapper>>,
        create_info: &vk::RenderPassCreateInfo,
        name: &str
    ) -> DeviceRenderpass {
        let renderpass = unsafe {
            device.borrow().get().create_render_pass(create_info, None)
                .expect("Failed to create renderpass")
        };

        device.borrow().set_debug_name(vk::ObjectType::RENDER_PASS, renderpass.as_raw(), name);

        DeviceRenderpass {
            renderpass,
            device
        }
    }

    pub fn push_debug_label(
        &self,
        command_buffer: vk::CommandBuffer,
        label: &str) {
        unsafe {
            let c_label = CString::new(label)
                .expect("Failed to create C-string for debug label");
            let debug_label = DebugUtilsLabelEXT::builder()
                .label_name(&c_label)
                .build();
            self.debug_utils.cmd_begin_debug_utils_label(command_buffer, &debug_label);
        }
    }

    pub fn pop_debug_label(
        &self,
        command_buffer: vk::CommandBuffer) {
        unsafe {
            self.debug_utils.cmd_end_debug_utils_label(command_buffer);
        }
    }
}

#[derive(Clone)]
pub struct DeviceShader {
    pub shader_module: vk::ShaderModule,
    pub device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DeviceShader {
    fn drop(&mut self) {
        unsafe {
            self.device.borrow().get().destroy_shader_module(self.shader_module, None)
        }
    }
}

impl DeviceShader {
    pub fn new(shader_module: vk::ShaderModule, device: Rc<RefCell<DeviceWrapper>>) -> Self {
        DeviceShader {
            shader_module,
            device
        }
    }
}

#[derive(Clone)]
pub struct DevicePipeline {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pub device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DevicePipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.borrow().get().destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.borrow().get().destroy_pipeline(self.pipeline, None);
            for dsl in &self.descriptor_set_layouts {
                self.device.borrow().get().destroy_descriptor_set_layout(*dsl, None);
            }
        }
    }
}

impl DevicePipeline {
    pub fn new(
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        device: Rc<RefCell<DeviceWrapper>>) -> Self {

        DevicePipeline {
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            device
        }
    }
}

#[derive(Clone)]
pub struct DeviceRenderpass {
    pub renderpass: vk::RenderPass,
    pub device: Rc<RefCell<DeviceWrapper>>
}

impl Drop for DeviceRenderpass {
    fn drop(&mut self) {
        unsafe {
            self.device.borrow().get().destroy_render_pass(self.renderpass, None);
        }
    }
}

impl DeviceRenderpass {
    pub fn new(
        renderpass: vk::RenderPass,
        device: Rc<RefCell<DeviceWrapper>>) -> Self {

        DeviceRenderpass {
            renderpass,
            device
        }
    }
}
