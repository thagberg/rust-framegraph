use std::collections::HashMap;
use crate::pass_node::PassNode;
use crate::resource::resource_manager::{ResourceManager};
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResourceCreateInfo, VulkanResourceManager};

use context::render_context::{RenderContext, CommandBuffer};

use ash::vk;
use context::vulkan_render_context::VulkanRenderContext;
use crate::graphics_pass_node::GraphicsPassNode;

pub trait RenderpassManager {
    type PN;
    type RM;
    type RC;
    type RP;

    fn create_or_fetch_renderpass(
        &mut self,
        pass_node: &Self::PN,
        resource_manager: &Self::RM,
        render_context: &Self::RC
    ) -> Self::RP;
}

pub struct VulkanRenderpassManager {
    renderpass_map: HashMap<String, vk::RenderPass>
}

impl RenderpassManager for VulkanRenderpassManager {
    type PN = GraphicsPassNode;
    type RM = VulkanResourceManager;
    type RC = VulkanRenderContext;
    type RP = vk::RenderPass;

    fn create_or_fetch_renderpass(
        &mut self,
        pass_node: &Self::PN,
        resource_manager: &Self::RM,
        render_context: &Self::RC) -> Self::RP {

        *self.renderpass_map.entry(pass_node.get_name().to_string()).or_insert({
            // no cached renderpass found, create it and cache it now
            let mut color_attachments: Vec<vk::AttachmentDescription> = Vec::new();
            let mut attachment_refs: Vec<vk::AttachmentReference> = Vec::new();
            for (i, render_target) in pass_node.get_rendertargets().into_iter().enumerate() {
                match resource_manager.get_resource_description(render_target) {
                    Some(create_info) => {
                        match create_info {
                            ResourceCreateInfo::Image(rt_description) => {
                                let rt_create_info = rt_description.get_create_info();
                                color_attachments.push(vk::AttachmentDescription::builder()
                                    .format(rt_create_info.format)
                                    .samples(rt_create_info.samples)
                                    .initial_layout(rt_create_info.initial_layout)
                                    .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)// TODO: this needs to be paramateried
                                    .load_op(vk::AttachmentLoadOp::CLEAR)
                                    .store_op(vk::AttachmentStoreOp::STORE)
                                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                                    .build());
                                attachment_refs.push(vk::AttachmentReference::builder()
                                    .attachment(i as u32)
                                    .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                                    .build());
                            },
                            ResourceCreateInfo::Buffer(_) => {
                                panic!("Expected image description, found buffer instead")
                            }
                        }
                    },
                    None => {
                        panic!("RenderpassManager could not find description for rendertarget")
                    }
                }
            }

            let subpass = vk::SubpassDescription::builder()
                .color_attachments(&attachment_refs)
                .flags(vk::SubpassDescriptionFlags::empty())
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

            let subpass_dependency = vk::SubpassDependency::builder()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::MEMORY_WRITE) // TODO: confirm this
                .src_stage_mask(vk::PipelineStageFlags::TOP_OF_PIPE)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dependency_flags(vk::DependencyFlags::empty());

            let renderpass_create_info = vk::RenderPassCreateInfo::builder()
                .flags(vk::RenderPassCreateFlags::empty())
                .attachments(&color_attachments)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(std::slice::from_ref(&subpass_dependency)).build();

            render_context.create_renderpass(&renderpass_create_info)
        })
    }
}

impl VulkanRenderpassManager {
    pub fn new() -> Self {
        VulkanRenderpassManager {
            renderpass_map: HashMap::new()
        }
    }

}

// let render_target = render_context.create_transient_image(
// vk::ImageCreateInfo::builder()
// .image_type(vk::ImageType::TYPE_2D)
// .format(vk::Format::R8G8B8A8_SRGB)
// .sharing_mode(vk::SharingMode::EXCLUSIVE)
// // .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
// .initial_layout(vk::ImageLayout::UNDEFINED)
// .samples(vk::SampleCountFlags::TYPE_1)
// .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
// .extent(vk::Extent3D::builder().height(100).width(100).depth(1).build())
// .mip_levels(1)
// .array_layers(1)
// .build()
//
// let color_attachment = vk::AttachmentDescription::builder()
// .format(vk::Format::R8G8B8A8_SRGB)
// .flags(vk::AttachmentDescriptionFlags::empty())
// .samples(vk::SampleCountFlags::TYPE_1)
// .load_op(vk::AttachmentLoadOp::CLEAR)
// .store_op(vk::AttachmentStoreOp::STORE)
// .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
// .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
// .initial_layout(vk::ImageLayout::UNDEFINED)
// .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
