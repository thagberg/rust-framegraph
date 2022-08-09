use ash::vk;

pub trait RenderPassCreate {

}

pub trait RenderPass {

}

pub struct VulkanRenderPassCreate(vk::RenderPassCreateInfo);

impl RenderPassCreate for VulkanRenderPassCreate {}

impl std::ops::Deref for VulkanRenderPassCreate {
    type Target = vk::RenderPassCreateInfo;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Copy, Clone)]
pub struct VulkanRenderPass(pub vk::RenderPass);

impl RenderPass for VulkanRenderPass {}

impl std::ops::Deref for VulkanRenderPass {
    type Target = vk::RenderPass;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

