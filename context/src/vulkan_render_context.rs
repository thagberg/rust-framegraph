use std::cell::RefCell;
use std::char::MAX;
use std::ffi::{c_void, CStr};
use std::ffi::CString;
use std::os::raw::c_char;
use std::rc::Rc;
use ash::{vk};
use ash::vk::{DebugUtilsMessengerEXT, PresentModeKHR};
use ash::extensions::ext::DebugUtils;
use winit::window::Window;

use ash::vk::DebugUtilsMessageSeverityFlagsEXT as severity_flags;
use ash::vk::DebugUtilsMessageTypeFlagsEXT as type_flags;

use crate::api_types::device::{QueueFamilies, PhysicalDeviceWrapper, DeviceWrapper, DeviceFramebuffer, DeviceResource, VulkanDebug};
use crate::api_types::swapchain::SwapchainWrapper;
use crate::api_types::image::ImageWrapper;
use crate::api_types::surface::SurfaceWrapper;
use crate::api_types::instance::InstanceWrapper;
use crate::render_context::RenderContext;

const MAX_FRAMES_IN_FLIGHT: u32 = 2;

unsafe extern "system" fn debug_utils_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    p_user_data: *mut c_void
) -> vk::Bool32 {
    let severity = match severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "[Verbose]",
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "[Warning]",
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "[Error]",
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "[Info]",
        _ => "[Unknown]",
    };
    let types = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
        _ => "[Unknown]",
    };

    let message = CStr::from_ptr((*p_callback_data).p_message);
    println!("[Debug]{}{}{:?}", severity, types, message);

    vk::FALSE
}

fn create_vulkan_instance(
    entry: &ash::Entry,
    application_info: &vk::ApplicationInfo,
    required_layer_names: &[&CStr],
    required_extension_names: &[&CStr]) -> ash::Instance {

    // let layer_names_raw: Vec<CString> = required_layer_names
    //     .iter()
    //     .

    let raw_layer_names: Vec<*const c_char> = required_layer_names
        .iter()
        .map(|layer_name| layer_name.as_ptr())
        .collect();

    let raw_extension_names: Vec<*const c_char> = required_extension_names
        .iter()
        .map(|extension_name| extension_name.as_ptr())
        .collect();

    let mut builder = vk::InstanceCreateInfo::builder()
        .application_info(&application_info)
        .enabled_layer_names(&raw_layer_names)
        .enabled_extension_names(&raw_extension_names);

    let mut instance_debug = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(severity_flags::WARNING | severity_flags::ERROR)
        .message_type(type_flags::GENERAL | type_flags::PERFORMANCE | type_flags::VALIDATION)
        .pfn_user_callback(Some(debug_utils_callback))
        .build();

    if required_layer_names.len() > 0 {
        builder = builder.push_next(&mut instance_debug);
    }

    let instance = unsafe {
        entry.create_instance(&builder, None)
            .expect("Failed to create Vulkan Instance")
    };

    instance
}

fn get_queue_family_indices(
    instance: &InstanceWrapper,
    physical_device: vk::PhysicalDevice,
    surface: &Option<SurfaceWrapper>
) -> QueueFamilies {
    let queue_families = unsafe {
        instance.get().get_physical_device_queue_family_properties(physical_device)
    };

    let mut queue_family_indices = QueueFamilies {graphics: None, compute: None, present: None};

    let mut current_index: u32 = 0;
    for queue_family in queue_families.iter() {
        if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            queue_family_indices.graphics = Some(current_index);
        }

        if queue_family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
            queue_family_indices.compute = Some(current_index);
        }

        let is_present_supported = {
            match surface {
                Some(surface) => {
                    unsafe {
                        surface.get_loader().get_physical_device_surface_support(
                            physical_device,
                            current_index,
                            surface.get_surface()
                        ).is_ok()
                    }
                },
                None => {
                    false
                }
            }
        };
        if is_present_supported {
            queue_family_indices.present = Some(current_index);
        }

        if queue_family_indices.graphics.is_some() &&
            queue_family_indices.compute.is_some() &&
            (queue_family_indices.present.is_some() || surface.is_none()) {
            break;
        }

        current_index += 1;
    }

    queue_family_indices
}

pub fn are_extensions_supported(
    instance: &InstanceWrapper,
    physical_device: vk::PhysicalDevice,
    required_extensions: &[&CStr]
) -> bool {
    // let available_extensions: Vec<&CStr> = unsafe {
    let extension_properties;
    let mut available_extensions = unsafe {
        extension_properties = instance.get().enumerate_device_extension_properties(physical_device)
        .expect("Failed to enumerate extensions from physical device.");

        extension_properties
        .iter()
        .map(|extension| {
            CStr::from_ptr(extension.extension_name.as_ptr())
        })
        // .collect()
    };


    for extension in required_extensions.iter() {
        let found = available_extensions.find(|available| {
            available.eq(extension)
        });

        if found.is_none() {
            return false;
        }
    }

    true
}

fn is_physical_device_suitable(
    physical_device: vk::PhysicalDevice,
    instance: &InstanceWrapper,
    surface: &Option<SurfaceWrapper>,
    required_extensions: &[&CStr] ) -> bool {

    let queue_families = get_queue_family_indices(instance, physical_device, surface);
    let extensions_supported = are_extensions_supported(
        instance,
        physical_device,
        required_extensions);

    match surface {
        Some(_) => {
            queue_families.is_complete() && extensions_supported
        },
        None => {
            queue_families.graphics.is_some() &&
                queue_families.compute.is_some() &&
                extensions_supported
        }
    }
}

fn pick_physical_device(
    instance: &InstanceWrapper,
    surface: &Option<SurfaceWrapper>,
    required_extensions: &[&CStr]) -> Result<PhysicalDeviceWrapper, &'static str> {

    let devices = unsafe {
        instance.get()
            .enumerate_physical_devices()
            .expect("Error enumerating physical devides")
    };

    let result = devices.iter().find(|device| {
        let is_suitable = is_physical_device_suitable(
            **device,
            instance,
            surface,
            required_extensions
        );

        is_suitable
    });

    match result {
        Some(physical_device) => Ok(PhysicalDeviceWrapper::new(*physical_device)),
        None => Err("No suitable device found.")
    }

}

fn create_logical_device(
    instance: &InstanceWrapper,
    debug: Option<VulkanDebug>,
    physical_device: &PhysicalDeviceWrapper,
    surface: &Option<SurfaceWrapper>,
    layers: &[&CStr],
    extensions: &[&CStr]
) -> DeviceWrapper {
    let queue_family_indices = get_queue_family_indices(
        instance,
        physical_device.get(),
        surface);

    // queue family indices could be overlapping (i.e. graphics and compute on the same family)
    // thus we want to ensure we're only creating one queue per family
    // Investigate explicitly creating separate queues even if one family supports all
    use std::collections::HashSet;
    let mut unique_family_indices = HashSet::new();
    unique_family_indices.insert(queue_family_indices.graphics.unwrap());
    unique_family_indices.insert(queue_family_indices.compute.unwrap());
    if queue_family_indices.present.is_some() {
        unique_family_indices.insert(queue_family_indices.present.unwrap());
    }

    let priorities = [1.0_f32];
    let mut queue_create_infos = vec![];
    for &family_index in unique_family_indices.iter() {
        let queue_create_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(family_index)
            .queue_priorities(&priorities)
            .build();
        queue_create_infos.push(queue_create_info);
    }

    let physical_device_features = vk::PhysicalDeviceFeatures {
        ..Default::default()
    };

    // convert layer names to const char*
    let p_layers: Vec<*const c_char> = layers.iter().map(|c_layer| {
        c_layer.as_ptr()
    }).collect();

    // do the same for extensions
    let p_extensions: Vec<*const c_char> = extensions.iter().map(|c_extension| {
        c_extension.as_ptr()
    }).collect();

    let device_create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_infos)
        .enabled_layer_names(&p_layers)
        .enabled_extension_names(&p_extensions)
        .enabled_features(&physical_device_features)
        .build();

    let device = unsafe {
        instance.get().create_device(physical_device.get(), &device_create_info, None)
            .expect("Failed to create logical device.")
    };

    DeviceWrapper::new(device, instance.get(), &physical_device, debug, queue_family_indices)
}

fn create_command_pool(
    device: &DeviceWrapper,
    queue_family_index: u32
) -> vk::CommandPool {
    let create_info = vk::CommandPoolCreateInfo {
        s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        queue_family_index
    };

    unsafe {
        device.get().create_command_pool(&create_info, None)
            .expect("Failed to create graphics command pool.")
    }
}

fn create_command_buffers(
    device: &DeviceWrapper,
    command_pool: vk::CommandPool,
    num_command_buffers: u32) -> Vec<vk::CommandBuffer> {
    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_buffer_count(num_command_buffers)
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .build();

    unsafe {
        device.get().allocate_command_buffers(&command_buffer_allocate_info)
            .expect("Failed to allocate Command Buffers")
    }
}

fn create_swapchain(
    instance: &InstanceWrapper,
    device: Rc<RefCell<DeviceWrapper>>,
    physical_device: &PhysicalDeviceWrapper,
    surface: &SurfaceWrapper,
    window: &winit::window::Window
) -> SwapchainWrapper {
    let swapchain_capabilities = surface.get_surface_capabilities(physical_device, surface);

    // TODO: may want to make format and color space customizable
    let swapchain_format = {
        let mut chosen_format: Option<vk::SurfaceFormatKHR> = None;
        for format in &swapchain_capabilities.formats {
            if format.format == vk::Format::R8G8B8A8_SRGB &&
                format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR {
                // break format.clone();
                chosen_format = Some(format.clone());
                break;
            }
        }

        if chosen_format.is_none() {
            // TODO: pick better than just the first format available
            chosen_format = Some(swapchain_capabilities.formats.first().unwrap().clone());
        }

        chosen_format.unwrap()
    };

    let swapchain_present_mode = {
        let mut chosen_mode: Option<PresentModeKHR> = None;
        for present_mode in swapchain_capabilities.present_modes {
            if present_mode == vk::PresentModeKHR::IMMEDIATE {
                chosen_mode = Some(present_mode);
                break;
            }
        }

        if chosen_mode.is_none() {
            chosen_mode = Some(vk::PresentModeKHR::FIFO);
        }

        chosen_mode.unwrap()
    };

    let swapchain_extent = {
        use num::clamp;
        let window_size = window.inner_size();
        let caps = &swapchain_capabilities.capabilities;
        vk::Extent2D {
            width: clamp(
                window_size.width,
                caps.min_image_extent.width,
                caps.max_image_extent.width
            ),
            height: clamp(
                window_size.height,
                caps.min_image_extent.height,
                caps.max_image_extent.height
            )
        }
    };

    // just assume double-buffering for now
    let image_count = MAX_FRAMES_IN_FLIGHT;

    // TODO: using exclusive mode right now but might want to make this concurrent
    let image_sharing_mode = vk::SharingMode::EXCLUSIVE;

    let create_info = vk::SwapchainCreateInfoKHR {
        s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
        p_next: std::ptr::null(),
        flags: vk::SwapchainCreateFlagsKHR::empty(),
        surface: surface.get_surface(),
        min_image_count: image_count,
        image_color_space: swapchain_format.color_space,
        image_format: swapchain_format.format,
        image_extent: swapchain_extent,
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
        image_sharing_mode,
        queue_family_index_count: 0,
        p_queue_family_indices: std::ptr::null(),
        pre_transform: swapchain_capabilities.capabilities.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        present_mode: swapchain_present_mode,
        clipped: vk::TRUE,
        old_swapchain: vk::SwapchainKHR::null(),
        image_array_layers: 1
    };

    let swapchain_loader = ash::extensions::khr::Swapchain::new(
        instance.get(),
        device.borrow().get());
    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&create_info, None)
            .expect("Failed to create swapchain.")
    };

    let swapchain_images : Vec<Rc<RefCell<DeviceResource>>> = unsafe {
        swapchain_loader
            .get_swapchain_images(swapchain)
            .expect("Failed to get swapchain images.")
            .iter()
            .map(|image| {
                Rc::new(RefCell::new(DeviceWrapper::wrap_image(
                    device.clone(),
                    image.clone(),
                    swapchain_format.format,
                    vk::ImageAspectFlags::COLOR,
                    1,
                    vk::Extent3D {
                        width: swapchain_extent.width,
                        height: swapchain_extent.height,
                        depth: 1
                    },
                    true)))
            })
            .collect()
    };

    SwapchainWrapper::new(
        swapchain_loader,
        swapchain,
        swapchain_images,
        swapchain_format.format,
        swapchain_extent)
}

pub struct VulkanFrameObjects {
    pub graphics_command_buffer: vk::CommandBuffer,
    pub swapchain_image: Option<Rc<RefCell<DeviceResource>>>,
    pub swapchain_semaphore: vk::Semaphore,
    pub descriptor_pool: vk::DescriptorPool,
    pub frame_index: u32
}

pub struct VulkanRenderContext {
    frame_index: u32,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    compute_queue: vk::Queue,
    graphics_command_pool: vk::CommandPool,
    graphics_command_buffers: Vec<vk::CommandBuffer>,
    immediate_command_buffer: vk::CommandBuffer,
    descriptor_pool: vk::DescriptorPool,
    swapchain: Option<SwapchainWrapper>,
    swapchain_semaphores: Vec<vk::Semaphore>,
    device: Rc<RefCell<DeviceWrapper>>,
    physical_device: PhysicalDeviceWrapper,
    surface: Option<SurfaceWrapper>,
    instance: InstanceWrapper,
    entry: ash::Entry
}

impl Drop for VulkanRenderContext {
    fn drop(&mut self) {
        unsafe {
            let device = self.device.borrow();
            for semaphore in &self.swapchain_semaphores {
                device.get().destroy_semaphore(*semaphore, None);
            }
            device.get().free_command_buffers(self.graphics_command_pool, &[self.immediate_command_buffer]);
            device.get().free_command_buffers(self.graphics_command_pool, &self.graphics_command_buffers);
            device.get().destroy_command_pool(self.graphics_command_pool, None);
            device.get().destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}

impl RenderContext for VulkanRenderContext {
    type Create = vk::RenderPassCreateInfo;
    type RP = vk::RenderPass;

    fn get_device(&self) -> Rc<RefCell<DeviceWrapper>> { self.device.clone() }

}

impl VulkanRenderContext {
    pub fn new(
        application_info: &vk::ApplicationInfo,
        debug_enabled: bool,
        window: Option<&winit::window::Window>
    ) -> VulkanRenderContext {
        // let extensions = vec!(ash::extensions::khr::Swapchain::name().as_ptr());

        let layers = [
            unsafe { ::std::ffi::CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") }
        ];

        let instance_extensions = [
            ash::extensions::ext::DebugUtils::name(),
            ash::extensions::khr::Win32Surface::name(),
            ash::extensions::khr::Surface::name()];

        let device_extensions = [
            ash::extensions::khr::Swapchain::name()
        ];

        let entry = ash::Entry::linked();
        let instance = create_vulkan_instance(
            &entry,
            application_info,
            &layers,
            &instance_extensions);

        let debug = {
            if debug_enabled {
                let debug_utils_loader = ash::extensions::ext::DebugUtils::new(&entry, &instance);
                let messenger = unsafe {
                    debug_utils_loader.create_debug_utils_messenger(
                        &vk::DebugUtilsMessengerCreateInfoEXT::builder()
                            .message_severity(severity_flags::WARNING | severity_flags::ERROR)
                            .message_type(type_flags::GENERAL | type_flags::PERFORMANCE | type_flags::VALIDATION)
                            .pfn_user_callback(Some(debug_utils_callback))
                            .build(),
                        None)
                        .expect("Failed to create Debug Utils Messenger")
                };
                Some(VulkanDebug{
                    debug_utils: debug_utils_loader,
                    debug_messenger: messenger,
                })
            } else {
                None
            }
        };

        let surface_wrapper = {
            match window {
                Some(win) => {
                    Some(SurfaceWrapper::new(
                        &entry,
                        &instance,
                        win
                    ))
                }
                None => {
                    None
                }
            }
        };

        let instance_wrapper = InstanceWrapper::new(instance);

        let physical_device = pick_physical_device(
    &instance_wrapper,
            &surface_wrapper,
        &device_extensions).expect("Failed to select a suitable physical device.");

        let logical_device = Rc::new(RefCell::new(create_logical_device(
            &instance_wrapper,
            debug,
            &physical_device,
            &surface_wrapper,
            &layers,
            &device_extensions
        )));

        let graphics_queue = unsafe {
            logical_device.borrow().get().get_device_queue(
                logical_device.borrow().get_queue_family_indices().graphics.unwrap(),
                0)
        };
        let present_queue = unsafe {
            logical_device.borrow().get().get_device_queue(
                logical_device.borrow().get_queue_family_indices().present.unwrap(),
                0)
        };
        let compute_queue = unsafe {
            logical_device.borrow().get().get_device_queue(
                logical_device.borrow().get_queue_family_indices().compute.unwrap(),
                0)
        };

        let graphics_command_pool = create_command_pool(
            &logical_device.borrow(),
            logical_device.borrow().get_queue_family_indices().graphics.unwrap());

        let graphics_command_buffers = create_command_buffers(
            &logical_device.borrow(),
            graphics_command_pool,
            MAX_FRAMES_IN_FLIGHT);

        let immediate_command_buffer = create_command_buffers(
            &logical_device.borrow(),
            graphics_command_pool,
            1);

        let swapchain = {
            if window.is_some() && surface_wrapper.is_some() {
                Some(create_swapchain(
                    &instance_wrapper,
                    logical_device.clone(),
                    &physical_device,
                    &surface_wrapper.as_ref().unwrap(),
                    window.unwrap()))
            } else {
                None
            }
        };

        let swapchain_semaphores = {
            let mut semaphores: Vec<vk::Semaphore> = Vec::new();
            if let Some(swapchain) = &swapchain {
                semaphores.reserve(swapchain.get_images().len());
                for i in 0..swapchain.get_images().len() {
                    let create_info = vk::SemaphoreCreateInfo::builder()
                        .build();

                    semaphores.push(unsafe {
                        logical_device.borrow().get().create_semaphore(&create_info, None)
                            .expect("Failed to create semaphore for swapchain image")
                    });
                }
            }

            semaphores
        };


        let ubo_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 8
        };
        let image_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::INPUT_ATTACHMENT,
            descriptor_count: 8
        };
        let combined_sampler_pool_size = vk::DescriptorPoolSize::builder()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(8)
            .build();
        let descriptor_pool_sizes = [ubo_pool_size, image_pool_size, combined_sampler_pool_size];
        let descriptor_pool_create = vk::DescriptorPoolCreateInfo::builder()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .max_sets(8)
            .pool_sizes(&descriptor_pool_sizes);
        let descriptor_pool = unsafe {
            logical_device.borrow().get().create_descriptor_pool(
                &descriptor_pool_create,
                None)
                .expect("Failed to create descriptor pool")
        };

        let frame_index = 0;

        VulkanRenderContext {
            entry,
            instance: instance_wrapper,
            device: logical_device,
            physical_device,
            graphics_queue,
            present_queue,
            compute_queue,
            graphics_command_pool,
            surface: surface_wrapper,
            swapchain,
            swapchain_semaphores,
            descriptor_pool,
            graphics_command_buffers,
            immediate_command_buffer: immediate_command_buffer[0],
            frame_index
        }
    }

    pub fn get_instance(&self) -> &ash::Instance {
        &self.instance.get()
    }

    pub fn get_physical_device(&self) -> &PhysicalDeviceWrapper { &self.physical_device }

    pub fn get_graphics_queue_index(&self) -> u32
    {
        self.device.borrow().get_queue_family_indices().graphics.unwrap()
    }

    pub fn get_graphics_queue(&self) -> vk::Queue {
        self.graphics_queue
    }

    pub fn get_present_queue(&self) -> vk::Queue {
        self.present_queue
    }

    pub fn get_graphics_command_pool(&self) -> vk::CommandPool { self.graphics_command_pool }

    fn get_graphics_command_buffer(&self, index: usize) -> vk::CommandBuffer { self.graphics_command_buffers[index] }

    pub fn get_immediate_command_buffer(&self) -> vk::CommandBuffer { self.immediate_command_buffer }

    pub fn get_swapchain(&self) -> &Option<SwapchainWrapper> { &self.swapchain }

    fn get_next_swapchain_image(
        &mut self,
        timeout: Option<u64>,
        semaphore: Option<vk::Semaphore>,
        fence: Option<vk::Fence>) -> Option<(Rc<RefCell<DeviceResource>>)> {

        match &mut self.swapchain {
            Some(swapchain) => {
                Some(swapchain.acquire_next_image(timeout, semaphore, fence))
            }
            None => {
                None
            }
        }
    }

    pub fn get_next_frame_objects(&mut self) -> VulkanFrameObjects {
        let old_index = self.frame_index;
        self.frame_index = (self.frame_index + 1) % MAX_FRAMES_IN_FLIGHT;

        let semaphore = self.swapchain_semaphores[old_index as usize];
        let image = self.get_next_swapchain_image(
            Some(std::time::Duration::new(1, 0).as_nanos() as u64),
            Some(semaphore),
            None);
        VulkanFrameObjects {
            graphics_command_buffer: self.graphics_command_buffers[old_index as usize],
            swapchain_image: image,
            swapchain_semaphore: semaphore,
            descriptor_pool: self.descriptor_pool, // TODO: this should be per-frame
            frame_index: old_index
        }
    }

    pub fn create_descriptor_sets(
        &self,
        layouts: &[vk::DescriptorSetLayout]) -> Vec<vk::DescriptorSet> {

        if layouts.len() > 0 {
            let alloc_info = vk::DescriptorSetAllocateInfo {
                s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
                p_next: std::ptr::null(),
                descriptor_pool: self.descriptor_pool,
                descriptor_set_count: layouts.len() as u32,
                p_set_layouts: layouts.as_ptr()
            };

            let descriptor_sets = unsafe {
                self.device.borrow().get().allocate_descriptor_sets(&alloc_info )
                    .expect("Failed to allocate descriptor sets")
            };

            return descriptor_sets;
        }

        Vec::new()
    }

    pub fn create_framebuffer(
        &self,
        render_pass: vk::RenderPass,
        extent: &vk::Extent3D,
        images: &[ImageWrapper]) -> DeviceFramebuffer {

        let image_views: Vec<vk::ImageView> = images.iter().map(|i| i.view).collect();

        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            .attachments(&image_views)
            .width(extent.width)
            .height(extent.height)
            .layers(extent.depth);

        unsafe {
            let framebuffer = self.device.borrow().get().create_framebuffer(&create_info, None)
                .expect("Failed to create framebuffer");
            DeviceFramebuffer::new(framebuffer, self.device.clone())
        }
    }

    pub fn submit_graphics(
        &self,
        command_buffers: &[vk::CommandBuffer],
        fence: vk::Fence,
        wait_semaphores: &[vk::Semaphore],
        signal_semaphores: &[vk::Semaphore]) {

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(std::slice::from_ref(&vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT))
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores)
            .build();

        unsafe {
            self.device.borrow().get()
                .queue_submit(
                    self.get_graphics_queue(),
                    std::slice::from_ref(&submit_info),
                    fence)
                .expect("Failed to execute Graphics submit");
        }
    }

    pub fn flip(
        &self,
        wait_semaphores: &[vk::Semaphore],
        image_index: u32) {

        if let Some(swapchain) = &self.swapchain {
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(wait_semaphores)
                .swapchains(&[swapchain.get()])
                .image_indices(&[image_index])
                .build();

            unsafe {
                swapchain.get_loader().queue_present(
                    self.get_present_queue(),
                    &present_info)
                    .expect("Failed to execute queue present");
            }
        } else {
            panic!("Attempted to flip without a swapchain");
        }
    }
}