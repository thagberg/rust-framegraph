use std::cell::RefCell;
use std::rc::Rc;

use ash::vk;
use gpu_allocator::MemoryLocation;

use context::api_types::device::{DeviceResource, DeviceWrapper};
use context::api_types::image::ImageCreateInfo;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::binding::{BindingInfo, BindingType, ImageBindingInfo, ResourceBinding};
use framegraph::compute_pass_node::ComputePassNode;
use framegraph::pass_type::PassType;

const BLUR_TARGET_CREATE_INFO: ImageCreateInfo = ImageCreateInfo::new(
    Default::default(),
    "blur_target".to_string());

pub fn generate_pass(
    device: Rc<RefCell<DeviceWrapper>>,
    source: Rc<RefCell<DeviceResource>>
) -> PassType {
    
    let source_binding = ResourceBinding {
        resource: source.clone(),
        binding_info: BindingInfo {
            binding_type: BindingType::Image(ImageBindingInfo {
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
            }),
            set: 0,
            slot: 0,
            stage: vk::PipelineStageFlags::COMPUTE_SHADER,
            access: vk::AccessFlags::SHADER_READ
        }
    };

    let blur_target = Rc::new(RefCell::new(DeviceWrapper::create_image(
        device,
        &BLUR_TARGET_CREATE_INFO,
        MemoryLocation::Unknown)));

    let target_binding = ResourceBinding {
        resource: blur_target,
        binding_info: BindingInfo {
            binding_type: BindingType::Image(ImageBindingInfo {
                layout: vk::ImageLayout::GENERAL
            }),
            set: 0,
            slot: 0,
            stage: Default::default(),
            access: Default::default()
        }
    };

    let pass_node = ComputePassNode::builder("blur".to_string())
        .input(source_binding)
        .output(target_binding)
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                  command_buffer: &vk::CommandBuffer | {

            }
        ))
        .build()
        .expect("Failed to create blur passnode");

    PassType::Compute(pass_node)
}