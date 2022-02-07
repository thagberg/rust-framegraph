use std::ptr::drop_in_place;
use std::thread::current;
use std::ffi::CStr;
use ash::vk;
use ash::vk::SwapchainImageUsageFlagsANDROID;

use crate::{
    InstanceWrapper,
    // PhysicalDeviceWrapper,
    DeviceWrapper,
    SurfaceWrapper};
use crate::api_types::device::PhysicalDeviceWrapper;

// type QueueFamilies = (Option<u32>, Option<u32>);
pub struct QueueFamilies {
    pub graphics: Option<u32>,
    pub compute: Option<u32>,
    pub present: Option<u32>
}

impl QueueFamilies {
    fn is_complete(&self) -> bool {
        self.graphics.is_some() && self.compute.is_some() && self.present.is_some()
    }
}

pub struct RenderContext {
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    surface: Option<SurfaceWrapper>,
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
        instance.get().get_physical_device_queue_family_properties(physical_device);
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
                        )
                    }
                },
                None => {
                    false
                }
            }
        };
        if is_present_supported.is_ok() {
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
    required_extensions: &[str]
) -> bool {
    let available_extensions: Vec<CStr> = unsafe {
        instance.get().enumerate_device_extension_properties(physical_device)
            .expect("Failed to enumerate extensions from physical device.")
            .iter()
            .map(|extension| {
                let raw_string = CStr::from_ptr(extension.extension_name.as_ptr());
                raw_string.to_str().expect("Failed to retrieve extension name")
            })
            .collect()
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
    required_extensions: &[str] ) -> bool {

    let device_features = unsafe {
        instance.get().get_physical_device_features(physical_device)
    };
    let queue_families = get_queue_family_indices(instance, physical_device, surface);
    let extensions_supported = are_extensions_supported(
        instance,
        physical_device,
        required_extensions);

    queue_families.is_complete() && extensions_supported

}

fn pick_physical_device(
    instance: &InstanceWrapper,
    surface: &Option<SurfaceWrapper>,
    required_extensions: &[str]) -> Result<PhysicalDeviceWrapper, &str> {

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
        Some(physical_device) => Ok(PhysicalDeviceWrapper::new(physical_device)),
        None => Err("No suitable device found.")
    }

}


impl RenderContext {
    pub fn new(
        entry: ash::Entry,
        instance: ash::Instance,
        // device: ash::Device,
        surface: Option<SurfaceWrapper>,
        // graphics_queue: vk::Queue,
        // present_queue: vk::Queue
    ) -> RenderContext {

        let instance_wrapper = InstanceWrapper::new(instance);
        let physical_device = pick_physical_device(
    &instance_wrapper,
            &surface,
            &[]).expect("Failed to select a suitable physical device.");

        RenderContext {
            entry,
            instance: instance_wrapper,
            device: DeviceWrapper::new(),
            physical_device,
            surface,
            graphics_queue,
            present_queue
        }
    }

    pub fn get_instance(&self) -> &ash::Instance {
        &self.instance.get()
    }

    pub fn get_device(&self) -> &ash::Device { &self.device.get() }

    pub fn get_graphics_queue(&self) -> vk::Queue {
        self.graphics_queue
    }

    pub fn get_present_queue(&self) -> vk::Queue {
        self.present_queue
    }
}