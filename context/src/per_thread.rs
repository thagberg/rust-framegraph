use std::sync::{Arc, Mutex};
use ash::vk;
use api_types::device::DeviceWrapper;

pub enum ThreadType {
    Main,
    Worker
}

pub struct PerThread {
    device: Arc<Mutex<DeviceWrapper>>,
    // TODO: how do I make this member private?
    thread_type: ThreadType,
    graphics_pool: vk::CommandPool,
    compute_pool: vk::CommandPool,
    pub descriptor_pool: vk::DescriptorPool,
    pub immediate_graphics_buffer: vk::CommandBuffer,
    pub graphics_command_buffers: Vec<vk::CommandBuffer>,
    pub compute_command_buffers: Vec<vk::CommandBuffer>
}

fn create_command_buffers(
    device: &DeviceWrapper,
    command_pool: vk::CommandPool,
    command_buffer_level: vk::CommandBufferLevel,
    num_command_buffers: u32) -> Vec<vk::CommandBuffer> {
    let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
        .command_buffer_count(num_command_buffers)
        .command_pool(command_pool)
        .level(command_buffer_level)
        .build();

    unsafe {
        device.get().allocate_command_buffers(&command_buffer_allocate_info)
            .expect("Failed to allocate Command Buffers")
    }
}

impl Drop for PerThread {
    fn drop(&mut self) {
        unsafe {
            let device = self.device.lock()
                .expect("Failed to obtain device lock.");

            device.get().free_command_buffers(self.graphics_pool, std::slice::from_ref(&self.immediate_graphics_buffer));
            device.get().free_command_buffers(self.graphics_pool, &self.graphics_command_buffers);
            device.get().free_command_buffers(self.compute_pool, &self.compute_command_buffers);

            device.get().destroy_command_pool(self.graphics_pool, None);
            device.get().destroy_command_pool(self.compute_pool, None);

            device.get().destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}

impl PerThread {
    pub fn new(
        device: Arc<Mutex<DeviceWrapper>>,
        thread_type: ThreadType,
        graphics_pool: vk::CommandPool,
        compute_pool: vk::CommandPool,
        descriptor_pool: vk::DescriptorPool,
        num_graphics_buffers: u32,
        num_compute_buffers: u32) -> Self {

        let device_ref = device.lock()
            .expect("Failed to obtain device lock while creating PerThread object");

        let command_buffer_level = match(thread_type) {
            ThreadType::Main => { vk::CommandBufferLevel::PRIMARY}
            ThreadType::Worker => { vk::CommandBufferLevel::SECONDARY}
        };

        let immediate_graphics_buffer = create_command_buffers(
            &device_ref,
            graphics_pool,
            command_buffer_level,
            1
        ).pop().expect("No command buffers were created for immediate command buffer");

        let graphics_command_buffers = create_command_buffers(
            &device_ref,
            graphics_pool,
            command_buffer_level,
            num_graphics_buffers
        );

        let compute_command_buffers = create_command_buffers(
            &device_ref,
            compute_pool,
            command_buffer_level,
            num_compute_buffers
        );

        drop(device_ref);

        PerThread {
            device: device.clone(),
            thread_type,
            graphics_pool,
            compute_pool,
            descriptor_pool,
            immediate_graphics_buffer,
            graphics_command_buffers,
            compute_command_buffers
        }
    }
}