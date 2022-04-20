use std::ptr::{drop_in_place, swap};
use std::thread::current;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use core::ffi::c_void;
use ash::vk;
use ash::vk::{Image, ImageView, PresentModeKHR, SwapchainImageUsageFlagsANDROID};
use winapi::um::wingdi::wglSwapMultipleBuffers;
use untitled::utility::share::find_queue_family;

use crate::{
    InstanceWrapper,
    // PhysicalDeviceWrapper,
    DeviceWrapper,
    SurfaceWrapper};
use crate::api_types::device::{QueueFamilies, PhysicalDeviceWrapper};
use crate::api_types::swapchain::SwapchainWrapper;
use crate::api_types::image::ImageWrapper;
use crate::api_types::surface::SurfaceCapabilities;
use crate::resource::resource_manager::{
    ResolvedBuffer,
    ResourceManager,
    ResourceHandle,
    ResolvedResource,
    ResourceCreateInfo
};

pub struct RenderContext {
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    compute_queue: vk::Queue,
    swapchain: Option<SwapchainWrapper>,
    swapchain_image_views: Option<Vec<vk::ImageView>>,
    surface: Option<SurfaceWrapper>,
    graphics_command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    resource_manager: ResourceManager,
    device: DeviceWrapper,
    physical_device: PhysicalDeviceWrapper,
    instance: InstanceWrapper,
    entry: ash::Entry
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
    required_extensions: &[&str]
) -> bool {
    // let available_extensions: Vec<&CStr> = unsafe {
    let mut extension_properties;
    let mut available_extensions = unsafe {
        extension_properties = instance.get().enumerate_device_extension_properties(physical_device)
        .expect("Failed to enumerate extensions from physical device.");

        extension_properties
        .iter()
        .map(|extension| {
            let raw_string = CStr::from_ptr(extension.extension_name.as_ptr());
            raw_string.to_str().expect("Failed to retrieve extension name")
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
    required_extensions: &[&str] ) -> bool {

    let device_features = unsafe {
        instance.get().get_physical_device_features(physical_device)
    };
    let queue_families = get_queue_family_indices(instance, physical_device, surface);
    let extensions_supported = are_extensions_supported(
        instance,
        physical_device,
        required_extensions);

    match surface {
        Some(surface) => {
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
    required_extensions: &[&str]) -> Result<PhysicalDeviceWrapper, &'static str> {

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
    physical_device: &PhysicalDeviceWrapper,
    surface: &Option<SurfaceWrapper>,
    layers: Vec<&str>,
    extensions: Vec<&str>
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
        let queue_create_info = vk::DeviceQueueCreateInfo {
            s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::DeviceQueueCreateFlags::empty(),
            queue_family_index: family_index,
            p_queue_priorities: priorities.as_ptr(),
            queue_count: priorities.len() as u32
        };
        queue_create_infos.push(queue_create_info);
    }

    let physical_device_features = vk::PhysicalDeviceFeatures {
        ..Default::default()
    };

    // convert layer names to const char*
    let c_layers : Vec<CString> = layers.iter().map(|layer| {
        CString::new(*layer).expect("Failed to translate layer name to C String")
    }).collect();
    let p_layers: Vec<*const c_char> = c_layers.iter().map(|c_layer| {
        c_layer.as_ptr()
    }).collect();

    // do the same for extensions
    let c_extensions : Vec<CString> = extensions.iter().map(|extension| {
        CString::new(*extension).expect("Failed to translate extension name to C String")
    }).collect();
    let p_extensions: Vec<*const c_char> = c_extensions.iter().map(|c_extension| {
        c_extension.as_ptr()
    }).collect();


    // TODO: implement layers and extensions
    let device_create_info = vk::DeviceCreateInfo {
        s_type: vk::StructureType::DEVICE_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: vk::DeviceCreateFlags::empty(),
        queue_create_info_count: queue_create_infos.len() as u32,
        p_queue_create_infos: queue_create_infos.as_ptr(),

        enabled_layer_count: layers.len() as u32,
        pp_enabled_layer_names: p_layers.as_ptr(),
        enabled_extension_count: extensions.len() as u32,
        pp_enabled_extension_names: p_extensions.as_ptr(),

        p_enabled_features: &physical_device_features
    };

    let device = unsafe {
        instance.get().create_device(physical_device.get(), &device_create_info, None)
            .expect("Failed to create logical device.")
    };

    DeviceWrapper::new(device, queue_family_indices)
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

fn create_swapchain(
    instance: &InstanceWrapper,
    device: &DeviceWrapper,
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
    let image_count = 2;

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
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
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
        device.get());
    let swapchain = unsafe {
        swapchain_loader
            .create_swapchain(&create_info, None)
            .expect("Failed to create swapchain.")
    };

    let mut swapchain_images = unsafe {
        swapchain_loader
            .get_swapchain_images(swapchain)
            .expect("Failed to get swapchain images.")
            .iter()
            .map(|image| {
                // image is just a handle type
                ImageWrapper::new(image.clone())
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

fn create_image_view(
    device: &DeviceWrapper,
    image: &ImageWrapper,
    format: vk::Format,
    image_view_flags: vk::ImageViewCreateFlags,
    aspect_flags: vk::ImageAspectFlags,
    mip_levels: u32
) -> vk::ImageView {
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
        image: image.get()
    };

    unsafe {
        device.get().create_image_view(&create_info, None)
            .expect("Failed to create image view.")
    }
}

impl RenderContext {
    pub fn new(
        entry: ash::Entry,
        instance: ash::Instance,
        surface: Option<SurfaceWrapper>,
        window: &winit::window::Window
    ) -> RenderContext {
        let layers = vec!("VK_LAYER_KHRONOS_validation");
        let extensions = vec!("VK_KHR_swapchain");
        // let extensions = vec!(ash::extensions::khr::Swapchain::name().as_ptr());

        let instance_wrapper = InstanceWrapper::new(instance);
        let physical_device = pick_physical_device(
    &instance_wrapper,
            &surface,
            &[]).expect("Failed to select a suitable physical device.");

        let logical_device = create_logical_device(
            &instance_wrapper,
            &physical_device,
            &surface,
            layers,
            extensions
        );

        let graphics_queue = unsafe {
            logical_device.get().get_device_queue(
                logical_device.get_queue_family_indices().graphics.unwrap(),
                0)
        };
        let present_queue = unsafe {
            logical_device.get().get_device_queue(
                logical_device.get_queue_family_indices().present.unwrap(),
                0)
        };
        let compute_queue = unsafe {
            logical_device.get().get_device_queue(
                logical_device.get_queue_family_indices().compute.unwrap(),
                0)
        };

        let graphics_command_pool = create_command_pool(
            &logical_device,
            logical_device.get_queue_family_indices().graphics.unwrap());

        let swapchain = {
            if surface.is_some() {
                Some(create_swapchain(
                    &instance_wrapper,
                    &logical_device,
                    &physical_device,
                    &surface.as_ref().unwrap(),
                    window))
            } else {
                None
            }
        };

        let swapchain_image_views: Option<Vec<vk::ImageView>> = {
            match &swapchain {
                Some(swapchain) => {
                    Some(swapchain.get_images().iter()
                        .map(|image| {
                            create_image_view(
                                &logical_device,
                                image,
                                swapchain.get_format(),
                                vk::ImageViewCreateFlags::empty(),
                                vk::ImageAspectFlags::COLOR,
                                1)
                        })
                        .collect())
                },
                _ => { None }
            }
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
        let descriptor_pool_create = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::DescriptorPoolCreateFlags::empty(),
            max_sets: 8,
            pool_size_count: descriptor_pool_sizes.len() as u32,
            p_pool_sizes: descriptor_pool_sizes.as_ptr()
        };
        let descriptor_pool = unsafe {
            logical_device.get().create_descriptor_pool(
                &descriptor_pool_create,
                None)
                .expect("Failed to create descriptor pool")
        };

        let resource_manager = ResourceManager::new(
            instance_wrapper.get(),
            &logical_device,
            &physical_device);

        RenderContext {
            entry,
            instance: instance_wrapper,
            device: logical_device,
            physical_device,
            surface,
            graphics_queue,
            present_queue,
            compute_queue,
            graphics_command_pool,
            swapchain,
            swapchain_image_views,
            descriptor_pool,
            resource_manager
        }
    }

    pub fn get_instance(&self) -> &ash::Instance {
        &self.instance.get()
    }

    pub fn get_device(&self) -> &ash::Device { &self.device.get() }

    pub fn get_physical_device(&self) -> vk::PhysicalDevice { self.physical_device.get() }

    pub fn get_graphics_queue(&self) -> vk::Queue {
        self.graphics_queue
    }

    pub fn get_present_queue(&self) -> vk::Queue {
        self.present_queue
    }

    pub fn get_graphics_command_pool(&self) -> vk::CommandPool { self.graphics_command_pool }

    pub fn get_swapchain(&self) -> &Option<SwapchainWrapper> { &self.swapchain }

    pub fn get_swapchain_image_views(&self) -> &Option<Vec<vk::ImageView>> { &self.swapchain_image_views }

    // pub fn create_uniform_buffer(&mut self, size: vk::DeviceSize) -> ResolvedBuffer {
    //     self.resource_manager.create_uniform_buffer(&self.device, size)
    // }
    pub fn create_buffer_persistent(&mut self, create_info: &vk::BufferCreateInfo) -> ResourceHandle {
        self.resource_manager.create_buffer_persistent(&self.device, create_info)
    }

    pub fn update_buffer_persistent<F>(&mut self, buffer_handle: &ResourceHandle, mut fill_callback: F)
        where F: FnMut(*mut c_void)
    {

        self.resource_manager.update_buffer(&self.device, buffer_handle, fill_callback);
    }

    pub fn create_transient_buffer(&mut self, create_info: vk::BufferCreateInfo) -> ResourceHandle
    {
        self.resource_manager.create_buffer_transient(create_info)
    }

    pub fn create_transient_image(&mut self, create_info: vk::ImageCreateInfo) -> ResourceHandle
    {
        self.resource_manager.create_image_transient(create_info)
    }

    pub fn resolve_resource(&self, handle: &ResourceHandle) -> ResolvedResource {
        self.resource_manager.resolve_resource(handle)
    }

    pub fn create_descriptor_sets(
        &self,
        layouts: &[vk::DescriptorSetLayout]) -> Vec<vk::DescriptorSet> {

        let alloc_info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
            p_next: std::ptr::null(),
            descriptor_pool: self.descriptor_pool,
            descriptor_set_count: layouts.len() as u32,
            p_set_layouts: layouts.as_ptr()
        };

        let descriptor_sets = unsafe {
            self.device.get().allocate_descriptor_sets(&alloc_info )
                .expect("Failed to allocate descriptor sets")
        };

        descriptor_sets
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        unsafe {
            self.device.get().destroy_command_pool(self.graphics_command_pool, None);

            if self.swapchain_image_views.is_some() {
                for &imageview in self.swapchain_image_views.as_ref().unwrap().iter() {
                    self.device.get().destroy_image_view(imageview, None);
                }
            }
        }
    }
}