use std::cell::RefCell;
use std::ffi::{c_void, CStr};
use std::fmt::{Debug, Formatter};
use std::os::raw::c_char;
use std::rc::Rc;
use ash::{vk};
use ash::vk::{ExtendsPhysicalDeviceFeatures2, PFN_vkGetPhysicalDeviceFeatures2, PhysicalDeviceFeatures2, PhysicalDeviceFeatures2Builder, PresentModeKHR};

use ash::vk::DebugUtilsMessageSeverityFlagsEXT as severity_flags;
use ash::vk::DebugUtilsMessageTypeFlagsEXT as type_flags;
use api_types::device::{DeviceFramebuffer, DeviceResource, DeviceWrapper, PhysicalDeviceWrapper, QueueFamilies, VulkanDebug};
use api_types::image::ImageWrapper;
use api_types::instance::InstanceWrapper;
use api_types::surface;
use api_types::surface::SurfaceWrapper;
use api_types::swapchain::{NextImage, SwapchainStatus, SwapchainWrapper};
use profiling::{enter_span, init_gpu_profiling, reset_gpu_profiling};

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

#[cfg(target_os = "macos")]
fn get_instance_extensions() -> Vec<&'static CStr> {
    // Need to support portability drivers for MoltenVK
    vec![
        vk::KhrPortabilityEnumerationFn::name(),
        vk::KhrGetPhysicalDeviceProperties2Fn::name(),
        vk::ExtSurfaceMaintenance1Fn::name(), // dependency of device extension EXTSwapchainMaintenance1
    ]
}

#[cfg(not(target_os = "macos"))]
fn get_instance_extensions() -> Vec<&'static CStr> {
    vec![
        vk::KhrGetPhysicalDeviceProperties2Fn::name(),
        vk::ExtSurfaceMaintenance1Fn::name(), // dependency of device extension EXTSwapchainMaintenance1
    ]
    // instance_extensions.push(vk::KhrPortabilityEnumerationFn::name());
    // instance_extensions.push(vk::KhrGetPhysicalDeviceProperties2Fn::name());
}

#[cfg(target_os = "macos")]
fn get_logical_device_extensions() -> Vec<&'static CStr> {
    vec![
        ash::extensions::khr::Swapchain::name(),
        // Needed for MoltenVK
        vk::KhrPortabilitySubsetFn::name()
    ]
}

#[cfg(not(target_os = "macos"))]
fn get_logical_device_extensions() -> Vec<&'static CStr> {
    vec![
        ash::extensions::khr::Swapchain::name()
    ]
}

fn get_physical_device_extensions() -> Vec<&'static CStr> {
    vec![
        ash::extensions::khr::Swapchain::name(),
        vk::ExtSwapchainMaintenance1Fn::name()  // required for present signaling

    ]
}

#[cfg(target_os = "macos")]
fn get_instance_flags() -> vk::InstanceCreateFlags {
    vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
}

#[cfg(not(target_os = "macos"))]
fn get_instance_flags() -> vk::InstanceCreateFlags {
    vk::InstanceCreateFlags::empty()
}

trait PhysicalDeviceFeatureChecker {
    fn add_feature<'a>(&'a mut self, device_features: vk::PhysicalDeviceFeatures2Builder<'a>) -> vk::PhysicalDeviceFeatures2Builder<'a>;

    fn check_feature(&self, device_features: &vk::PhysicalDeviceFeatures2) -> bool;
}

struct HostQueryResetPhysicalDeviceFeature {
    feature: vk::PhysicalDeviceHostQueryResetFeatures
}

impl HostQueryResetPhysicalDeviceFeature {
    pub fn new() -> Self {
        let feature = vk::PhysicalDeviceHostQueryResetFeatures::builder()
            .host_query_reset(true)
            .build();
        HostQueryResetPhysicalDeviceFeature {
            feature: feature
        }
    }
}

impl PhysicalDeviceFeatureChecker for HostQueryResetPhysicalDeviceFeature {
    fn add_feature<'a>(&'a mut self, device_features: PhysicalDeviceFeatures2Builder<'a>) -> vk::PhysicalDeviceFeatures2Builder<'a> {
        device_features.push_next(&mut self.feature)
    }

    fn check_feature(&self, device_features: &PhysicalDeviceFeatures2) -> bool {
        self.feature.host_query_reset > 0
    }
}

fn get_required_physical_device_features() -> Vec<Box<dyn PhysicalDeviceFeatureChecker>> {
    vec![
        Box::new(HostQueryResetPhysicalDeviceFeature::new())
    ]
}

fn create_vulkan_instance(
    entry: &ash::Entry,
    application_info: &vk::ApplicationInfo,
    required_layer_names: &[&CStr],
    required_extension_names: &[&CStr]) -> ash::Instance {

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
        .enabled_extension_names(&raw_extension_names)
        .flags(get_instance_flags());

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

    let mut required_features = get_required_physical_device_features();

    let mut physical_device_features = vk::PhysicalDeviceFeatures2::builder();
    for mut required_feature in &mut required_features {
        physical_device_features = required_feature.add_feature(physical_device_features);
    }

    let mut resolved_physical_device_features = physical_device_features.build();

    unsafe {
        instance.get().get_physical_device_features2(
            physical_device,
            &mut resolved_physical_device_features);
    }

    let mut required_features_supported = true;
    for mut required_feature in required_features {
        if !required_feature.check_feature(&resolved_physical_device_features) {
            required_features_supported = false;
        }
    }

    match surface {
        Some(_) => {
            queue_families.is_complete() && extensions_supported && required_features_supported
        },
        None => {
            queue_families.graphics.is_some() &&
                queue_families.compute.is_some() &&
                extensions_supported &&
                required_features_supported
        }
    }
}

fn pick_physical_device(
    instance: &InstanceWrapper,
    surface: &Option<SurfaceWrapper>,
    required_extensions: &[&CStr]) -> Result<PhysicalDeviceWrapper, &'static str> {

    let mut devices = unsafe {
        instance.get()
            .enumerate_physical_devices()
            .expect("Error enumerating physical devides")
    };

    // sort physical devices such that discrete GPUs are preferred
    unsafe {
        devices.sort_by(|a, b| {
            let a_properties = instance.get().get_physical_device_properties(a.clone());
            let b_properties = instance.get().get_physical_device_properties(b.clone());

            let get_device_ranking = |device_type: vk::PhysicalDeviceType| -> u32 {
                match device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                    vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                    _ => 3
                }
            };

            let a_rank = get_device_ranking(a_properties.device_type);
            let b_rank = get_device_ranking(b_properties.device_type);
            a_rank.cmp(&b_rank)
        });
    }

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

    let mut core_physical_device_features = vk::PhysicalDeviceFeatures::builder().build();
    let mut physical_device_features = vk::PhysicalDeviceFeatures2::builder();
    // TODO: make this an argument rather than a function call here
    let mut required_features = get_required_physical_device_features();
    for mut required_feature in &mut required_features {
        physical_device_features = required_feature.add_feature(physical_device_features);
    }

    let mut resolved_physical_device_features = physical_device_features.build();

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
        // .enabled_features(&physical_device_features)
        // .enabled_features(&core_physical_device_features)
        .push_next(&mut resolved_physical_device_features)
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

fn create_debug_util(
    entry: &ash::Entry,
    instance: &ash::Instance,
    severity: severity_flags,
    message_flags: type_flags) -> VulkanDebug {
    let debug_utils_loader = ash::extensions::ext::DebugUtils::new(&entry, &instance);

    let messenger = unsafe {
        debug_utils_loader.create_debug_utils_messenger(
            &vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(severity)
                .message_type(message_flags)
                .pfn_user_callback(Some(debug_utils_callback))
                .build(),
            None)
            .expect("Failed to create Debug Utils Messenger")
    };

    VulkanDebug{
        debug_utils: debug_utils_loader,
        debug_messenger: messenger,
    }
}

fn create_swapchain(
    instance: &InstanceWrapper,
    device: Rc<RefCell<DeviceWrapper>>,
    physical_device: &PhysicalDeviceWrapper,
    surface: &SurfaceWrapper,
    window: &winit::window::Window,
    old_swapchain: &Option<OldSwapchain>
) -> SwapchainWrapper {
    let swapchain_capabilities = surface.get_surface_capabilities(physical_device);

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

    let image_count = {
        if swapchain_capabilities.capabilities.min_image_count > MAX_FRAMES_IN_FLIGHT {
            swapchain_capabilities.capabilities.min_image_count
        } else {
            MAX_FRAMES_IN_FLIGHT
        }
    };

    // TODO: using exclusive mode right now but might want to make this concurrent
    let image_sharing_mode = vk::SharingMode::EXCLUSIVE;

    let old = match &old_swapchain {
        Some(old) => {old.swapchain.get()}
        None => {vk::SwapchainKHR::null()}
    };
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
        old_swapchain: old,
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

    let mut present_fences: Vec<vk::Fence> = Vec::new();
    unsafe {
        let fence_create = vk::FenceCreateInfo::builder()
            .flags(vk::FenceCreateFlags::SIGNALED)
            .build();
        for _ in 0..swapchain_images.len() {
            present_fences.push(
                device.borrow().get().create_fence(
                    &fence_create,
                    None
                )
                .expect("Failed to create Present fence")
            );
        }
    }

    SwapchainWrapper::new(
        device.clone(),
        swapchain_loader,
        swapchain,
        swapchain_images,
        swapchain_format.format,
        swapchain_extent,
        present_fences)
}

#[derive(Debug)]
pub struct OldSwapchain {
    pub swapchain: SwapchainWrapper,
    pub frame_index: u32

}

pub struct VulkanFrameObjects {
    pub graphics_command_buffer: vk::CommandBuffer,
    pub swapchain_image: Option<NextImage>,
    pub swapchain_semaphore: vk::Semaphore,
    pub descriptor_pool: vk::DescriptorPool,
    pub frame_index: u32
}


// swapchain_index must be independent from frame_index since it will "reset"
// whenever we recreate the swapchain
// Necessary for avoiding errors when specifying image indices in VkPresentInfoKHR
pub struct VulkanRenderContext {
    frame_index: u32,
    swapchain_index: u32,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    compute_queue: vk::Queue,
    graphics_command_pool: vk::CommandPool,
    graphics_command_buffers: Vec<vk::CommandBuffer>,
    immediate_command_buffer: vk::CommandBuffer,
    descriptor_pools: Vec<vk::DescriptorPool>,
    swapchain: Option<SwapchainWrapper>,
    old_swapchain: Option<OldSwapchain>,
    swapchain_semaphores: Vec<vk::Semaphore>,
    device: Rc<RefCell<DeviceWrapper>>,
    physical_device: PhysicalDeviceWrapper,
    surface: Option<SurfaceWrapper>,
    instance: InstanceWrapper,
    entry: ash::Entry
}

impl Debug for VulkanRenderContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanRenderContext")
            .finish()
    }
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
            for pool in &self.descriptor_pools {
                device.get().destroy_descriptor_pool(*pool, None);
            }
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
        let layers = [
            unsafe { ::std::ffi::CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") }
        ];

        let mut instance_extensions = vec![
            ash::extensions::ext::DebugUtils::name()
        ];

        if let Some(resolved_window) = window {
            let extensions = surface::get_required_surface_extensions(resolved_window);
            for extension in extensions {
                unsafe {
                    instance_extensions.push(CStr::from_ptr(*extension));
                }
            }
        }

        instance_extensions.append(&mut get_instance_extensions());

        let mut physical_device_extensions = get_physical_device_extensions();
        let mut logical_device_extensions = get_logical_device_extensions();

        let entry = ash::Entry::linked();
        let instance = create_vulkan_instance(
            &entry,
            application_info,
            &layers,
            &instance_extensions);

        let debug = {
            if debug_enabled {
                Some(create_debug_util(
                    &entry,
                    &instance,
                    severity_flags::WARNING | severity_flags::ERROR,
                    type_flags::GENERAL | type_flags::PERFORMANCE | type_flags::VALIDATION))
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
            &physical_device_extensions).expect("Failed to select a suitable physical device.");

        logical_device_extensions.append(&mut physical_device_extensions);

        let logical_device = Rc::new(RefCell::new(create_logical_device(
            &instance_wrapper,
            debug,
            &physical_device,
            &surface_wrapper,
            &layers,
            &logical_device_extensions
        )));

        let swapchain = {
            if window.is_some() && surface_wrapper.is_some() {
                Some(create_swapchain(
                    &instance_wrapper,
                    logical_device.clone(),
                    &physical_device,
                    &surface_wrapper.as_ref().unwrap(),
                    window.unwrap(),
                    &None))
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

        let max_frames_in_flight = {
            if let Some(swapchain) = &swapchain {
               swapchain.get_images().len() as u32
            } else {
                MAX_FRAMES_IN_FLIGHT
            }
        };


        let ubo_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 16
        };
        let image_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::INPUT_ATTACHMENT,
            descriptor_count: 16
        };
        let combined_sampler_pool_size = vk::DescriptorPoolSize::builder()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(16)
            .build();
        let descriptor_pool_sizes = [ubo_pool_size, image_pool_size, combined_sampler_pool_size];
        let descriptor_pool_create = vk::DescriptorPoolCreateInfo::builder()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .max_sets(8)
            .pool_sizes(&descriptor_pool_sizes);

        let mut descriptor_pools: Vec<vk::DescriptorPool> = Vec::new();
        for i in (0..max_frames_in_flight) {
            let descriptor_pool = unsafe {
                logical_device.borrow().get().create_descriptor_pool(
                    &descriptor_pool_create,
                    None)
                    .expect("Failed to create descriptor pool")
            };
            descriptor_pools.push(descriptor_pool);
        }

        let immediate_command_buffer = create_command_buffers(
            &logical_device.borrow(),
            graphics_command_pool,
            1);

        let graphics_command_buffers = create_command_buffers(
            &logical_device.borrow(),
            graphics_command_pool,
            max_frames_in_flight);

        let frame_index = 0;

        {
            let borrowed_device = logical_device.borrow();
            let num_frames = match &swapchain {
                None => { MAX_FRAMES_IN_FLIGHT }
                Some(swapchain) => {
                    swapchain.get_images().len() as u32
                }
            };

            let device_properties = unsafe {
                instance_wrapper.get().get_physical_device_properties(
                    physical_device.get().clone()
                )
            };

            init_gpu_profiling!(
                borrowed_device.get(),
                device_properties.limits.timestamp_period,
                &immediate_command_buffer[0],
                &graphics_queue,
                num_frames);
        }


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
            old_swapchain: None,
            swapchain_semaphores,
            descriptor_pools,
            graphics_command_buffers,
            immediate_command_buffer: immediate_command_buffer[0],
            frame_index,
            swapchain_index: 0,
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

    pub fn recreate_swapchain(
        &mut self,
        window: &winit::window::Window
    ) {
        match &self.surface {
            Some(surface) => {
                // Only rebuild the swapchain if we aren't already doing so
                if let None = &self.old_swapchain {
                    self.old_swapchain = Some(OldSwapchain {
                        swapchain: self.swapchain.take().unwrap(),
                        frame_index: self.frame_index
                    });
                    let new_swapchain = create_swapchain(
                        &self.instance,
                        self.device.clone(),
                        &self.physical_device,
                        surface,
                        window,
                        &self.old_swapchain);

                    self.swapchain = Some(new_swapchain);
                    self.swapchain_index = 0;
                }
            }
            None => {
                panic!("Attempting to recreate swapchain when no surface exists");
            }
        }
    }

    fn get_next_swapchain_image(
        &mut self,
        timeout: Option<u64>,
        semaphore: Option<vk::Semaphore>,
        fence: Option<vk::Fence>) -> Option<NextImage> {

        match &mut self.swapchain {
            Some(swapchain) => {
                Some(swapchain.acquire_next_image(timeout, semaphore, fence))
            }
            None => {
                None
            }
        }
    }

    #[tracing::instrument]
    pub fn get_next_frame_objects(&mut self) -> VulkanFrameObjects {
        let old_index = self.frame_index;

        let semaphore = self.swapchain_semaphores[old_index as usize];
        let image = self.get_next_swapchain_image(
            None,
            Some(semaphore),
            None);

        // successful swapchain image acquisition on the same frame index of when
        // we recreated the swapchain should indicate that the presentation engine
        // is no longer using the old swapchain
        if let Some(old_swapchain) = &self.old_swapchain {
            if old_swapchain.swapchain.can_destroy() {
                self.old_swapchain = None;
            }
        }

        VulkanFrameObjects {
            graphics_command_buffer: self.graphics_command_buffers[old_index as usize],
            swapchain_image: image,
            swapchain_semaphore: semaphore,
            descriptor_pool: self.descriptor_pools[old_index as usize], // TODO: this should be per-frame
            frame_index: old_index
        }
    }

    pub fn create_descriptor_sets(
        &self,
        layouts: &[vk::DescriptorSetLayout],
        descriptor_pool: vk::DescriptorPool) -> Vec<vk::DescriptorSet> {
        enter_span!(tracing::Level::TRACE, "Create Descriptorsets");

        if layouts.len() > 0 {
            let alloc_info = vk::DescriptorSetAllocateInfo {
                s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
                p_next: std::ptr::null(),
                descriptor_pool: descriptor_pool,
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
        images: &[ImageWrapper],
        depth: &Option<ImageWrapper>) -> DeviceFramebuffer {
        enter_span!(tracing::Level::TRACE, "Create framebuffer");

        let mut image_views: Vec<vk::ImageView> = Vec::new();
        image_views.reserve(images.len() + 1);

        if let Some(depth_attachment) = depth {
            image_views.push(depth_attachment.view);
        }

        for image in images {
            image_views.push(image.view);
        }

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

    #[tracing::instrument]
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

    #[tracing::instrument]
    pub fn flip(
        &self,
        wait_semaphores: &[vk::Semaphore]) -> SwapchainStatus {

        let swapchain = {
            match &self.swapchain {
                Some(swapchain) => {
                    swapchain
                }
                None => {
                    panic!("Attempted to flip without a swapchain");
                }
            }
        };


        let raw_swapchain = swapchain.get();
        let swapchain_index = self.swapchain_index;
        let mut present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(wait_semaphores)
            .swapchains(std::slice::from_ref(&raw_swapchain))
            .image_indices(std::slice::from_ref(&swapchain_index));

        // wait for and reset the presentation fence
        let present_fence = swapchain.get_present_fence(self.swapchain_index);
        unsafe {
            enter_span!(tracing::Level::TRACE, "Waiting for Present fence");
            self.device.borrow().get().wait_for_fences(
                std::slice::from_ref(&present_fence),
                true,
                u64::MAX )
                .expect("Failed to wait for Present fence");

            self.device.borrow().get().reset_fences(
                std::slice::from_ref(&present_fence)
            ).expect("Failed to reset Present fence");
        }
        let mut swapchain_fence = vk::SwapchainPresentFenceInfoEXT::builder()
            .fences(std::slice::from_ref(&present_fence))
            .build();

        let resolved_present_info = present_info.push_next(&mut swapchain_fence).build();

        let is_suboptimal = unsafe {
            swapchain.get_loader().queue_present(
                self.get_present_queue(),
                &resolved_present_info)
                .expect("Failed to execute queue present")
        };

        match is_suboptimal {
            true => {SwapchainStatus::Suboptimal}
            false => {SwapchainStatus::Ok}
        }
    }

    pub fn start_frame(&mut self, frame_index: u32) {
        let borrowed_device = self.device.borrow();
        reset_gpu_profiling!(borrowed_device.get());
    }

    pub fn end_frame(&mut self) {
        let max_frames_in_flight = {
            if let Some(swapchain) = &self.swapchain {
                swapchain.get_images().len() as u32
            } else {
                MAX_FRAMES_IN_FLIGHT
            }
        };
        self.swapchain_index = (self.swapchain_index + 1) % max_frames_in_flight;
        self.frame_index = (self.frame_index + 1) % max_frames_in_flight;
    }
}