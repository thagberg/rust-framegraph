use ash::vk;
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;
use crate::api_types::device::PhysicalDeviceWrapper;
use crate::DeviceWrapper;

pub enum ResourceHandle {
    Transient(u32),
    Persistent(u32)
}

pub struct ResolvedBuffer {
    buffer: vk::Buffer,
    allocation: Allocation
}

pub struct ResourceManager {
    allocator: Allocator
}

impl ResolvedBuffer {
    pub fn get(&self) -> vk::Buffer { self.buffer }
    pub fn get_allocation(&self) -> &Allocation { &self.allocation }
}

impl ResourceManager {
    pub fn new(
        instance: &ash::Instance,
        device: &DeviceWrapper,
        physical_device: &PhysicalDeviceWrapper
    ) -> ResourceManager {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.get().clone(),
            physical_device: physical_device.get(),
            debug_settings: Default::default(),
            buffer_device_address: false // TODO: what is this
        }).expect("Failed to create GPU memory allocator");

        ResourceManager {
            allocator
        }
    }

    pub fn create_uniform_buffer(
        &mut self,
        device: &DeviceWrapper,
        size: vk::DeviceSize
    ) -> ResolvedBuffer {
        let create_info = vk::BufferCreateInfo {
            s_type: vk::StructureType::BUFFER_CREATE_INFO,
            size,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let buffer = unsafe {
            device.get().create_buffer(&create_info, None)
                .expect("Failed to create uniform buffer")
        };
        let requirements = unsafe {
            device.get().get_buffer_memory_requirements(buffer)
        };

        let buffer_alloc = self.allocator.allocate(&AllocationCreateDesc {
            name: "Uniform Buffer Allocation",
            requirements: requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true
        }).expect("Failed to allocate memory for uniform buffer");

        unsafe {
            device.get().bind_buffer_memory(
                buffer,
                buffer_alloc.memory(),
                buffer_alloc.offset())
                .expect("Failed to bind uniform buffer to memory")
        };

        ResolvedBuffer {
            buffer,
            allocation: buffer_alloc
        }
    }
}