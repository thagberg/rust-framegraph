use ash::vk;

pub struct BufferCreateInfo<'m> {
    create_info: vk::BufferCreateInfo<'m>,
    name: String
}

impl<'m> BufferCreateInfo<'m> {
    pub fn new(create_info: vk::BufferCreateInfo<'m>, name: String) -> Self {
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
    // pub create_info: vk::BufferCreateInfo<'m>
}

unsafe impl Sync for BufferWrapper {}
unsafe impl Send for BufferWrapper {}

impl BufferWrapper {
    pub fn new(buffer: vk::Buffer) -> Self {
        BufferWrapper {
            buffer
        }
    }

    pub fn get(&self) -> vk::Buffer { self.buffer }
}