use ash::vk;

pub type ResourceHandle = i32;

pub struct TextureDesc {
    width: u32,
    height: u32,
    format: vk::Format
}

pub struct FrameGraph {

}