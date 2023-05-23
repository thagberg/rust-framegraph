use core::ffi::c_void;
use std::cell::RefCell;
use std::rc::Rc;

use ash::vk;
use gpu_allocator::MemoryLocation;

use context::api_types::image::ImageCreateInfo;
use context::api_types::buffer::BufferCreateInfo;
use context::api_types::device::{DeviceResource, DeviceWrapper};
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;

use framegraph::binding::{BindingInfo, BindingType, BufferBindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::{GraphicsPassNode};
use framegraph::pipeline::{PipelineDescription, RasterizationType, DepthStencilType, BlendType};

pub struct OffsetUBO {
    pub offset: [f32; 3]
}

pub struct UBOPass {
    uniform_buffer: Rc<RefCell<DeviceResource>>
}

impl Drop for UBOPass {
    fn drop(&mut self) {

    }
}

impl UBOPass {
    pub fn new(
        device: Rc<RefCell<DeviceWrapper>>) -> Self {
        let ubo_create_info = BufferCreateInfo::new(
            vk::BufferCreateInfo {
                s_type: vk::StructureType::BUFFER_CREATE_INFO,
                size: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize,
                usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            },
            "ubo_buffer".to_string());

        let uniform_buffer = DeviceWrapper::create_buffer(
            device.clone(),
            &ubo_create_info,
            MemoryLocation::CpuToGpu);

        let ubo_value = OffsetUBO {
            offset: [0.2, 0.1, 0.0]
        };

        // TODO: pretty sure count should actually just be 1 here
        device.borrow().update_buffer(&uniform_buffer, |mapped_memory: *mut c_void, _size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &ubo_value,
                    mapped_memory as *mut OffsetUBO,
                    std::mem::size_of::<OffsetUBO>() );
            };
        });

        UBOPass {
            uniform_buffer: Rc::new(RefCell::new(uniform_buffer))
        }
    }

    pub fn generate_pass(
        &self,
        device: Rc<RefCell<DeviceWrapper>>,
        rendertarget_extent: vk::Extent2D) -> (GraphicsPassNode, Rc<RefCell<DeviceResource>>) {

        let vertex_input_state_create_info = vk::PipelineVertexInputStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::PipelineVertexInputStateCreateFlags::empty(),
            vertex_attribute_description_count: 0,
            p_vertex_attribute_descriptions: std::ptr::null(),
            vertex_binding_description_count: 0,
            p_vertex_binding_descriptions: std::ptr::null(),
        };

        let dynamic_states = vec!(vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR);

        let pipeline_description = PipelineDescription::new(
            vertex_input_state_create_info,
            dynamic_states,
            RasterizationType::Standard,
            DepthStencilType::Disable,
            BlendType::None,
            "hello-vert.spv",
            "hello-frag.spv"
        );

        // let color_attachment = create_color_attachment_transient(image_description);

        let render_target = Rc::new(RefCell::new(DeviceWrapper::create_image(
            device,
        &ImageCreateInfo::new(
            vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_SRGB)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                // .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .samples(vk::SampleCountFlags::TYPE_1)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST)
                .extent(vk::Extent3D::builder()
                    .height(rendertarget_extent.height)
                    .width(rendertarget_extent.width)
                    .depth(1)
                    .build())
                .mip_levels(1)
                .array_layers(1)
                .build(),
            "ubo_rendertarget".to_string()),
            MemoryLocation::GpuOnly)));

        let rt_ref = AttachmentReference::new(
            render_target.clone(),
            vk::Format::R8G8B8A8_SRGB,
            vk::SampleCountFlags::TYPE_1,
            vk::AttachmentLoadOp::CLEAR,
            vk::AttachmentStoreOp::STORE);

        let ubo_binding = ResourceBinding {
            resource: self.uniform_buffer.clone(),
            binding_info: BindingInfo {
                binding_type: BindingType::Buffer(BufferBindingInfo {
                    offset: 0,
                    range: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize
                }),
                set: 0,
                slot: 0,
                stage: vk::PipelineStageFlags::ALL_GRAPHICS,
                access: vk::AccessFlags::SHADER_READ
            }
        };

        let passnode = GraphicsPassNode::builder("ubo_pass".to_string())
            .pipeline_description(pipeline_description)
            .read(ubo_binding)
            .render_target(rt_ref)
            .fill_commands(Box::new(
                move |render_ctx: &VulkanRenderContext,
                      command_buffer: &vk::CommandBuffer|
                    {
                        println!("I'm doing something!");
                        let viewport = vk::Viewport::builder()
                            .x(0.0)
                            .y(0.0)
                            .width(1200.0)
                            .height(900.0)
                            .min_depth(0.0)
                            .max_depth(1.0)
                            .build();

                        let scissor = vk::Rect2D::builder()
                            .offset(vk::Offset2D{x: 0, y: 0})
                            .extent(vk::Extent2D::builder().width(1200).height(900).build())
                            .build();

                        unsafe {
                            render_ctx.get_device().borrow().get().cmd_set_viewport(
                                *command_buffer,
                                0,
                                std::slice::from_ref(&viewport));

                            render_ctx.get_device().borrow().get().cmd_set_scissor(
                                *command_buffer,
                                0,
                                std::slice::from_ref(&scissor));

                            render_ctx.get_device().borrow().get().cmd_draw(*command_buffer, 3, 1, 0, 0);
                        }
                    }
            ))
            .build()
            .expect("Failed to create PassNode");

        return (passnode, render_target.clone());
    }
}