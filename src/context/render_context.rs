use std::ptr::drop_in_place;
use std::thread::current;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use ash::vk;
use ash::vk::SwapchainImageUsageFlagsANDROID;
use winapi::um::wingdi::wglSwapMultipleBuffers;
use untitled::utility::share::find_queue_family;

use crate::{
    InstanceWrapper,
    // PhysicalDeviceWrapper,
    DeviceWrapper,
    SurfaceWrapper};
use crate::api_types::device::{QueueFamilies, PhysicalDeviceWrapper};

pub struct RenderContext {
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    compute_queue: vk::Queue,
    surface: Option<SurfaceWrapper>,
    graphics_command_pool: vk::CommandPool,
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
        flags: vk::CommandPoolCreateFlags::empty(),
        queue_family_index
    };

    unsafe {
        device.get().create_command_pool(&create_info, None)
            .expect("Failed to create graphics command pool.")
    }
}


impl RenderContext {
    pub fn new(
        entry: ash::Entry,
        instance: ash::Instance,
        surface: Option<SurfaceWrapper>
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

        RenderContext {
            entry,
            instance: instance_wrapper,
            device: logical_device,
            physical_device,
            surface,
            graphics_queue,
            present_queue,
            compute_queue,
            graphics_command_pool
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
}