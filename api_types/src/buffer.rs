use ash::vk;

pub struct BufferCreateInfo {
    create_info: vk::BufferCreateInfo,
    name: String
}

impl BufferCreateInfo {
    pub fn new(create_info: vk::BufferCreateInfo, name: String) -> Self {
        BufferCreateInfo {
            create_info,
            name
        }
    }

    pub fn get_create_info(&self) -> &vk::BufferCreateInfo { &self.create_info }

    pub fn get_name(&self) -> &str { &self.name }
}

#[derive(Clone)]
pub struct BufferWrapper {
    pub buffer: vk::Buffer
}

impl BufferWrapper {
    pub fn new(buffer: vk::Buffer) -> BufferWrapper {
        BufferWrapper {
            buffer
        }
    }

    pub fn get(&self) -> vk::Buffer { self.buffer }
}