use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use context::render_context::{RenderContext};

use ash::{Device, vk};
use context::api_types::device::{DeviceRenderpass, DeviceWrapper};
use context::vulkan_render_context::VulkanRenderContext;
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

pub fn create_renderpass(
    pass_name: &str,
    color_attachments: &[AttachmentReference],
    device: Rc<RefCell<DeviceWrapper>>) -> DeviceRenderpass {

    let mut color_attachment_descs: Vec<vk::AttachmentDescription> = Vec::new();
    let mut attachment_refs: Vec<vk::AttachmentReference> = Vec::new();

    let mut attachment_index = 0;
    for color_attachment in color_attachments {
        let mut load_op = vk::AttachmentLoadOp::LOAD;
        if (color_attachment.layout == vk::ImageLayout::UNDEFINED) {
            load_op = vk::AttachmentLoadOp::DONT_CARE;
        }
        color_attachment_descs.push(vk::AttachmentDescription::builder()
            .format(color_attachment.format)
            .samples(color_attachment.samples)
            .load_op(load_op)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(color_attachment.layout)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build());
        attachment_refs.push(vk::AttachmentReference::builder()
            .attachment(attachment_index)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build());
        attachment_index += 1;
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

    DeviceWrapper::create_renderpass(device, &renderpass_create_info, pass_name)
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
        device: Rc<RefCell<DeviceWrapper>>) -> Rc<RefCell<DeviceRenderpass>> {

        let renderpass = self.renderpass_map.entry(pass_name.to_string()).or_insert_with_key(|_| {
            // no cached renderpass found, create it and cache it now
            // Rc::new(RefCell::new(DeviceWrapper::create_renderpass(device, &renderpass_create_info, pass_name)))
            Rc::new(RefCell::new(create_renderpass(pass_name, color_attachments, device)))

        }).clone();
        renderpass
    }
}