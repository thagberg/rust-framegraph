use std::collections::HashMap;
use std::collections::vec_deque::VecDeque;
use crate::pass_node::PassNode;
use crate::resource::resource_manager::{ResourceManager};
use crate::resource::vulkan_resource_manager::{ResourceHandle, ResourceCreateInfo, VulkanResourceManager};

use context::render_context::{RenderContext, CommandBuffer};

use ash::vk;
use context::vulkan_render_context::VulkanRenderContext;
use crate::graphics_pass_node::GraphicsPassNode;

pub struct StencilAttachmentInfo {
    pub stencil_load_op: vk::AttachmentLoadOp,
    pub stencil_store_op: vk::AttachmentStoreOp
}

pub struct AttachmentInfo {
    pub samples: vk::SampleCountFlags,
    pub format: vk::Format,
    pub layout: vk::ImageLayout,
    pub load_op: vk::AttachmentLoadOp,
    pub store_op: vk::AttachmentStoreOp,
    pub stencil_attachment: Option<StencilAttachmentInfo>
}

pub struct PassAttachment {
    pub attachment: VecDeque<AttachmentInfo>
}

pub trait RenderpassManager {
    type PN;
    type RC;
    type RP;

    fn create_or_fetch_renderpass(
        &mut self,
        pass_node: &Self::PN,
        color_attachments: &mut [PassAttachment],
        render_context: &Self::RC
    ) -> Self::RP;
}

pub struct VulkanRenderpassManager {
    renderpass_map: HashMap<String, vk::RenderPass>
}

impl RenderpassManager for VulkanRenderpassManager {
    type PN = GraphicsPassNode;
    type RC = VulkanRenderContext;
    type RP = vk::RenderPass;

    fn create_or_fetch_renderpass(
        &mut self,
        pass_node: &Self::PN,
        color_attachments: &mut [PassAttachment],
        render_context: &Self::RC) -> Self::RP {

        *self.renderpass_map.entry(pass_node.get_name().to_string()).or_insert_with_key(|pass_name| {
            // no cached renderpass found, create it and cache it now
            let mut color_attachment_descs: Vec<vk::AttachmentDescription> = Vec::new();
            let mut attachment_refs: Vec<vk::AttachmentReference> = Vec::new();

            let mut attachment_index = 0;
            for color_attachment in color_attachments {
                let attachment_info = color_attachment.attachment.pop_front();
                let peek_info = color_attachment.attachment.front();

                if let Some(attachment_info) = attachment_info {
                    let next_layout = {
                        if let Some(peek_info) = peek_info {
                            peek_info.layout
                        }
                        else {
                            vk::ImageLayout::UNDEFINED
                        }
                    };
                    color_attachment_descs.push(vk::AttachmentDescription::builder()
                        .format(attachment_info.format)
                        .samples(attachment_info.samples)
                        .load_op(attachment_info.load_op)
                        .store_op(attachment_info.store_op)
                        .initial_layout(attachment_info.layout)
                        .final_layout(next_layout)
                        .build());
                    attachment_refs.push(vk::AttachmentReference::builder()
                        .attachment(attachment_index)
                        .layout(attachment_info.layout)
                        .build());
                }
                else {
                    panic!("Empty color attachment was provided")
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
                .attachments(&color_attachment_descs)
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