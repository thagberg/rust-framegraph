use std::ptr::drop_in_place;
use ash::vk;
use ash::vk::SwapchainImageUsageFlagsANDROID;

use crate::{InstanceWrapper, DeviceWrapper, SurfaceWrapper};

pub struct RenderContext {
    // instance: ash::Instance,
    // device: ash::Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    surface: Option<SurfaceWrapper>,
    device: DeviceWrapper,
    instance: InstanceWrapper,
    entry: ash::Entry
}

impl RenderContext {
    pub fn new(
        entry: ash::Entry,
        instance: ash::Instance,
        device: ash::Device,
        surface: Option<SurfaceWrapper>,
        graphics_queue: vk::Queue,
        present_queue: vk::Queue) -> RenderContext {

        // let instance_wrapper = InstanceWrapper::new(instance);
        // let device_wrapper = DeviceWrapper::new(device);

        RenderContext {
            entry,
            // instance: instance_wrapper,
            // device: device_wrapper,
            instance: InstanceWrapper::new(instance),
            device: DeviceWrapper::new(device),
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

// impl Drop for RenderContext {
//     fn drop(&mut self) {
//         unsafe {
//             match (&mut self.surface) {
//                 Some(surface) => {
//                     // std::mem::drop(surface);
//                     drop_in_place(surface);
//                 },
//                 _ => {}
//             }
//
//             self.device.destroy_device(None);
//             self.instance.destroy_instance(None);
//         }
//     }
// }