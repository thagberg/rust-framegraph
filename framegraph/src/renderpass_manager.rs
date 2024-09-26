use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use std::sync::Arc;

use ash::{vk};
use api_types::device::{DeviceRenderpass, DeviceWrapper};
use profiling::enter_span;
use crate::attachment::AttachmentReference;

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

pub struct VulkanRenderpassManager {
    renderpass_map: HashMap<String, Rc<RefCell<DeviceRenderpass>>>
}

impl Debug for VulkanRenderpassManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanRenderpassManager")
            .field("num renderpasses", &self.renderpass_map.len())
            .finish()
    }
}

impl VulkanRenderpassManager {

    pub fn new() -> Self {
        VulkanRenderpassManager {
            renderpass_map: HashMap::new()
        }
    }

    pub fn create_or_fetch_renderpass(
        &mut self,
        pass_name: &str,
        color_attachments: &[AttachmentReference],
        depth_attachment: &Option<AttachmentReference>,
        device: Arc<RefCell<DeviceWrapper>>) -> Rc<RefCell<DeviceRenderpass>> {
        enter_span!(tracing::Level::TRACE, "Create or Fetch Renderpass");

        let renderpass = self.renderpass_map.entry(pass_name.to_string()).or_insert_with_key(|_| {
            // no cached renderpass found, create it and cache it now
            let mut attachment_descs: Vec<vk::AttachmentDescription> = Vec::new();
            let mut color_attachment_refs: Vec<vk::AttachmentReference> = Vec::new();
            let mut depth_ref: Option<vk::AttachmentReference> = None;

            let mut attachment_index = 0;
            // We (potentially) add the depth target as the first attachment in case
            // we execute a depth-only draw
            if let Some(depth_attachment) = depth_attachment {
                // assert_eq!(depth_attachment.layout, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL, "Invalid layout for depth attachment");
                // attachment_refs.push(vk::AttachmentReference::builder()
                let mut load_op = vk::AttachmentLoadOp::LOAD;
                if (depth_attachment.layout == vk::ImageLayout::UNDEFINED) {
                    load_op = vk::AttachmentLoadOp::DONT_CARE;
                }

                attachment_descs.push(vk::AttachmentDescription::builder()
                    .format(depth_attachment.format)
                    .samples(depth_attachment.samples)
                    .load_op(load_op)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .initial_layout(depth_attachment.layout)
                    // TODO: add support for separateDepthStencilLayouts
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .build());

                depth_ref = Some(vk::AttachmentReference::builder()
                    .attachment(attachment_index)
                    // TODO: add support for separateDepthStencilLayouts
                    // .layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                    .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .build());
                attachment_index += 1;
            }

            for color_attachment in color_attachments {
                let mut load_op = vk::AttachmentLoadOp::LOAD;
                if (color_attachment.layout == vk::ImageLayout::UNDEFINED) {
                    load_op = vk::AttachmentLoadOp::DONT_CARE;
                }
                attachment_descs.push(vk::AttachmentDescription::builder()
                    .format(color_attachment.format)
                    .samples(color_attachment.samples)
                    .load_op(load_op)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .initial_layout(color_attachment.layout)
                    .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .build());
                color_attachment_refs.push(vk::AttachmentReference::builder()
                    .attachment(attachment_index)
                    .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .build());
                attachment_index += 1;
            }

            let mut subpass = vk::SubpassDescription::builder()
                .color_attachments(&color_attachment_refs)
                .flags(vk::SubpassDescriptionFlags::empty())
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);
            if let Some(depth_ref) = &depth_ref {
                subpass = subpass.depth_stencil_attachment(depth_ref);
            }

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
                .attachments(&attachment_descs)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(std::slice::from_ref(&subpass_dependency)).build();

            Rc::new(RefCell::new(DeviceWrapper::create_renderpass(device, &renderpass_create_info, pass_name)))
        }).clone();
        renderpass
    }
}