use ash::vk;
use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc};
use crate::device::physical::PhysicalDeviceWrapper;

pub struct ResourceAllocator {
    allocator: Allocator
}

impl Drop for ResourceAllocator {
    fn drop(&mut self) {
        self.allocator.report_memory_leaks(log::Level::Warn);
    }
}

impl ResourceAllocator {
    pub fn new(
        device: ash::Device,
        instance: &ash::Instance,
        physical_device: &PhysicalDeviceWrapper
    ) -> Self {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false, // https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPhysicalDeviceBufferDeviceAddressFeaturesEXT.html
            allocation_sizes: Default::default(), // TODO: optimize allocation block sizes?
        }).expect("Failed to create GPU memory allocator");

        ResourceAllocator {
            allocator
        }
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
            linear,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        }).expect("Failed to allocate memory for resource")
    }

    pub fn free_allocation(&mut self, allocation: Allocation) {
        self.allocator.free(allocation)
            .expect("Failed to free Device allocation");
    }
}