use crate::api_types::renderpass::{RenderPass, RenderPassCreate};

pub trait RenderContext  {
    type Create;
    type RP;

    fn create_renderpass(&self, create_info: &Self::Create) -> Self::RP;

    fn get_device(&self) -> &ash::Device;
}

pub trait CommandBuffer {

}
