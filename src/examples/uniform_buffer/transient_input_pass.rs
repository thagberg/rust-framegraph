use ash::vk;

use crate::context::render_context::RenderContext;
use crate::framegraph::pass_node::{PassNodeBuilder, PassNode};
use crate::resource::resource_manager::{ResourceType, ResourceHandle, ResolvedResource};

pub struct TransientInputPass {
    // pub pass_node: PassNode
}

impl TransientInputPass {
    pub fn new(render_context: &mut RenderContext, render_pass: vk::RenderPass) -> Self {
        // create descriptor set layouts
        let texture_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .immutable_samplers(&[]);
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(std::slice::from_ref(&texture_binding));
            // .binding_count(texture_bindings.len() as u32)
            // .p_bindings(texture_bindings.as_ptr());

        // create descriptor sets

        // create shader modules

        // create pipeline layout

        // create graphics pipeline

        // cleanup shader modules

        // create PassNode

        TransientInputPass {

        }
    }
}