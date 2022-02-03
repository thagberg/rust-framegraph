use ash::vk;

pub struct RenderContext {
    device: ash::Device,
    graphics_queue: vk::Queue,
    present_queue: vk::Queue
}

impl RenderContext {
    pub fn new(
        device: ash::Device,
        graphics_queue: vk::Queue,
        present_queue: vk::Queue) -> RenderContext {

        RenderContext {
            device,
            graphics_queue,
            present_queue
        }
    }

    pub fn get_device(&self) -> &ash::Device {
        &self.device
    }

    pub fn get_graphics_queue(&self) -> vk::Queue {
        self.graphics_queue
    }

    pub fn get_present_queue(&self) -> vk::Queue {
        self.present_queue
    }
}