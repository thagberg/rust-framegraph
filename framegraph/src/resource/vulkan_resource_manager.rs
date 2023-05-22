// use std::collections::HashMap;
// use core::ffi::c_void;
// use std::cell::RefCell;
// use std::rc::Rc;
// use ash::{Device, vk};
// use gpu_allocator::vulkan::*;
// use gpu_allocator::MemoryLocation;
//
// extern crate context;
// use context::api_types::device::{PhysicalDeviceWrapper, DeviceWrapper, ResourceType};
// use context::api_types::image::{ImageCreateInfo, ImageWrapper};
// use context::api_types::buffer::{BufferCreateInfo, BufferWrapper};
//
// use crate::resource::resource_manager::{ResourceManager};
//
// pub type ResourceHandle = u32;
//
// pub enum ResourceCreateInfo {
//     Buffer(BufferCreateInfo),
//     Image(ImageCreateInfo)
// }
//
// #[derive(Clone)]
// pub struct ResolvedResource {
//     pub handle: ResourceHandle,
//     pub resource: ResourceType
// }
//
// struct ResolvedResourceInternal {
//     resource: ResourceType,
//     allocation: Allocation
// }
//
// pub type ResolvedResourceMap = HashMap<ResourceHandle, ResolvedResource>;
// type ResolvedResourceInternalMap = HashMap<ResourceHandle, ResolvedResourceInternal>;
//
// pub struct VulkanResourceManager {
//     next_handle: RefCell<u32>,
//     allocator: Allocator,
//     resource_map: RefCell<ResolvedResourceInternalMap>,
//     /// Registered resources are those created / managed elsewhere but for which we still want
//     /// a resource handle and to be resolveable (such as swapchain images)
//     registered_resource_map: ResolvedResourceMap,
//     device: Rc<DeviceWrapper>
// }
//
// impl ResourceManager for VulkanResourceManager {
//     fn resolve_resource(
//         &self,
//         handle: &ResourceHandle) -> ResolvedResource
//     {
//         let map_ref = self.resource_map.borrow();
//         let found = map_ref.get(handle);
//         if let Some(found_resource) = found {
//             ResolvedResource {
//                 handle: *handle,
//                 resource: found_resource.resource.clone()
//             }
//         } else {
//             self.registered_resource_map.get(handle)
//                 .expect("Attempted to resolve a resource which doesn't exist")
//                 .clone()
//         }
//     }
//
//     fn reset(&mut self, device: &DeviceWrapper) {
//         for (handle, resolved) in self.resource_map.borrow_mut().drain() {
//             match &resolved.resource {
//                 ResourceType::Image(resolved_image) => {
//                     unsafe {
//                         device.get().destroy_image_view(resolved_image.view, None);
//                         device.get().destroy_image(resolved_image.image, None);
//                         self.allocator.free(resolved.allocation)
//                             .expect("Failed to free image allocation");
//                     }
//                 },
//                 ResourceType::Buffer(resolved_buffer) => {
//                     unsafe {
//                         device.get().destroy_buffer(resolved_buffer.buffer, None);
//                         self.allocator.free(resolved.allocation)
//                             .expect("Failed to free buffer allocation");
//                     }
//                 }
//             }
//         }
//     }
// }
//
// impl VulkanResourceManager {
//     pub fn new(
//         instance: &ash::Instance,
//         device: Rc<DeviceWrapper>,
//         physical_device: &PhysicalDeviceWrapper
//     ) -> VulkanResourceManager {
//         let allocator = Allocator::new(&AllocatorCreateDesc {
//             instance: instance.clone(),
//             device: device.get().clone(),
//             physical_device: physical_device.get(),
//             debug_settings: Default::default(),
//             buffer_device_address: false // TODO: what is this
//         }).expect("Failed to create GPU memory allocator");
//
//         VulkanResourceManager {
//             next_handle: RefCell::new(0u32),
//             allocator,
//             resource_map: RefCell::new(HashMap::new()),
//             registered_resource_map: HashMap::new(),
//             device
//         }
//     }
//
//     fn increment_handle(&self) -> ResourceHandle {
//         let mut handle = self.next_handle.borrow_mut();
//         let ret_handle = *handle;
//         *handle += 1;
//         ret_handle
//     }
//
//     pub fn reserve_handle(&self) -> ResourceHandle {
//         self.increment_handle()
//     }
//
//     pub fn free_resource(&mut self, handle: ResourceHandle) {
//         let removed = self.resource_map.borrow_mut().remove(&handle);
//         if let Some(removed_resource) = removed {
//             match removed_resource.resource {
//                 ResourceType::Image(removed_image) => {
//                     unsafe {
//                         self.device.get().destroy_image_view(removed_image.view, None);
//                         self.device.get().destroy_image(removed_image.image, None);
//                         self.allocator.free(removed_resource.allocation)
//                             .expect("Failed to free image allocation");
//                     }
//                 },
//                 ResourceType::Buffer(removed_buffer) => {
//                     unsafe {
//                         self.device.get().destroy_buffer(removed_buffer.buffer, None);
//                         self.allocator.free(removed_resource.allocation)
//                             .expect("Failed to free buffer allocation");
//                     }
//                 }
//             }
//         } else {
//             panic!("Attempted to remove resource that doesn't exist");
//         }
//     }
//
//     pub fn register_image(
//         &mut self,
//         image: &ImageWrapper,
//         name: &str
//     ) -> ResourceHandle
//     {
//         let ret_handle = self.increment_handle();
//
//         self.registered_resource_map.insert(
//             ret_handle,
//             ResolvedResource {
//                 handle: ret_handle,
//                 resource: ResourceType::Image(image.clone())
//             });
//
//         self.device.set_image_name(image, name);
//
//         ret_handle
//     }
//
//
// }