use std::cell::RefCell;
use std::rc::Rc;

use ash::vk;
use gpu_allocator::MemoryLocation;
use api_types::device::{DeviceResource, DeviceWrapper};
use api_types::image::{ImageCreateInfo, ImageType};

use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::binding::{BindingInfo, BindingType, ImageBindingInfo, ResourceBinding};
use framegraph::compute_pass_node::ComputePassNode;
use framegraph::pass_type::PassType;
use framegraph::pipeline::ComputePipelineDescription;
use profiling::{enter_gpu_span, enter_span};

pub fn generate_pass(
    device: Rc<RefCell<DeviceWrapper>>,
    source: Rc<RefCell<DeviceResource>>
) -> (PassType, Rc<RefCell<DeviceResource>>) {

    let image_extent = source.borrow().get_image().extent.clone();

    let blur_target_create_info: ImageCreateInfo = ImageCreateInfo::new(
        vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .extent(image_extent)
            .samples(vk::SampleCountFlags::TYPE_1)
            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC)
            .mip_levels(1)
            .array_layers(1)
            .build(),
        String::from("blur_target"),
        ImageType::Color);

    let source_binding = ResourceBinding {
        resource: source.clone(),
        binding_info: BindingInfo {
            binding_type: BindingType::Image(ImageBindingInfo {
                layout: vk::ImageLayout::GENERAL
            }),
            set: 0,
            slot: 0,
            stage: vk::PipelineStageFlags::COMPUTE_SHADER,
            access: vk::AccessFlags::SHADER_READ
        }
    };

    let blur_target = Rc::new(RefCell::new(DeviceWrapper::create_image(
        device,
        &blur_target_create_info,
        MemoryLocation::GpuOnly)));

    let target_binding = ResourceBinding {
        resource: blur_target.clone(),
        binding_info: BindingInfo {
            binding_type: BindingType::Image(ImageBindingInfo {
                layout: vk::ImageLayout::GENERAL
            }),
            set: 0,
            slot: 1,
            stage: vk::PipelineStageFlags::COMPUTE_SHADER,
            access: vk::AccessFlags::SHADER_WRITE
        }
    };

    let pipeline_description = ComputePipelineDescription::new("blur-comp.spv");

    let pass_node = ComputePassNode::builder("blur".to_string())
        .pipeline_description(pipeline_description)
        .input(source_binding)
        .output(target_binding)
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                  command_buffer: &vk::CommandBuffer | {

                enter_span!(tracing::Level::TRACE, "Blur");
                let device = render_ctx.get_device();
                let borrowed_device = device.borrow();
                enter_gpu_span!("Blur GPU", "Passes", borrowed_device.get(), command_buffer, vk::PipelineStageFlags::ALL_GRAPHICS);

                unsafe {
                    render_ctx.get_device().borrow().get().cmd_dispatch(
                        *command_buffer,
                        image_extent.width / 8,
                        image_extent.height / 8,
                        1);
                }
            }
        ))
        .build()
        .expect("Failed to create blur passnode");

    (PassType::Compute(pass_node), blur_target.clone())
}