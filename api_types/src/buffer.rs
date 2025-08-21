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
pub struct BufferWrapper<'m> {
    pub buffer: vk::Buffer,
    pub create_info: vk::BufferCreateInfo<'m>
}

unsafe impl Sync for BufferWrapper<'_> {}
unsafe impl Send for BufferWrapper<'_> {}

impl<'m> BufferWrapper<'m> {
    pub fn new(buffer: vk::Buffer, create_info: vk::BufferCreateInfo<'m>) -> Self {
        BufferWrapper {
            buffer,
            create_info
        }
    }

    pub fn get(&self) -> vk::Buffer { self.buffer }
}