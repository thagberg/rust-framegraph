use std::ops::Deref;
use std::sync::{Arc, Mutex, RwLock};
use ash::vk;
use crate::vulkan_descriptors::{DescriptorUpdate, resolve_descriptors};
use crate::pipeline::VulkanPipelineManager;
use crate::renderpass_manager::VulkanRenderpassManager;
use crate::graphics_pass_node::GraphicsPassNode;
use crate::compute_pass_node::ComputePassNode;
use crate::copy_pass_node::CopyPassNode;
use crate::pass_node::PassNode;
use api_types::device::interface::DeviceInterface;
use context::vulkan_render_context::VulkanRenderContext;
use context::render_context::RenderContext;
use profiling::enter_span;
use crate::attachment::AttachmentReference;
use api_types::device::resource::ResourceType;
use api_types::image::ImageWrapper;

pub fn resolve_render_targets(
    attachments: &[AttachmentReference]) -> Vec<ImageWrapper> {
    enter_span!(tracing::Level::TRACE, "Resolve RTs");

    let mut rts: Vec<ImageWrapper> = Vec::new();
    for attachment in attachments {
        let attachment_image = attachment.resource_image.lock().unwrap();
        let resolved = attachment_image.resource_type.as_ref().expect("Invalid rendertarget provided");
        if let ResourceType::Image(rt_image) = &resolved {
            // TODO: do I really want to copy the ImageWrappers here?
            rts.push(rt_image.clone());
        } else {
            panic!("A non-image resource was returned when attempting to resolve a render target");
        }
    }

    rts
}

#[tracing::instrument(skip(render_context, node))]
pub fn execute_copy_node(
    _descriptor_sets: Arc<RwLock<Vec<vk::DescriptorSet>>>,
    _descriptor_pool: vk::DescriptorPool,
    render_context: &VulkanRenderContext,
    command_buffer: &vk::CommandBuffer,
    node: &mut CopyPassNode) {

    // Copy node is ez-pz
    node.execute(
        render_context.get_device(),
        *command_buffer);
}

#[tracing::instrument(skip(pipeline_manager, device, node))]
pub fn execute_compute_node(
    pipeline_manager: &Mutex<VulkanPipelineManager>,
    _descriptor_sets: Arc<RwLock<Vec<vk::DescriptorSet>>>,
    _descriptor_pool: vk::DescriptorPool,
    device: DeviceInterface,
    command_buffer: vk::CommandBuffer,
    node: &mut ComputePassNode) {

    // get compute pipeline from node's pipeline description
    let pipeline = pipeline_manager.lock().unwrap().create_compute_pipeline(
        device.clone(),
        &node.pipeline_description);

    // bind pipeline
    let pipeline_ref = pipeline.lock().unwrap();
    {
        unsafe {
            device.get().cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::COMPUTE,
                pipeline_ref.get_pipeline());
        }

        // prepare and perform descriptor writes
        {
            let mut descriptor_updates = DescriptorUpdate::new();

            // get input and output handles for this pass
            // let inputs = node.get_inputs();
            let inputs = &node.inputs;
            let outputs = &node.outputs;

            resolve_descriptors(
                inputs,
                pipeline_ref.deref(),
                &[],
                &mut descriptor_updates);

            resolve_descriptors(
                outputs,
                pipeline_ref.deref(),
                &[],
                &mut descriptor_updates);

            let descriptor_writes = descriptor_updates.create_descriptor_writes();

            unsafe {
                // TODO: support descriptor copies?
                device.get().update_descriptor_sets(
                    &descriptor_writes,
                    &[]);
                // bind descriptorsets
                // TODO: COMPUTE SUPPORT
                device.get().cmd_bind_descriptor_sets(
                    command_buffer,
                    vk::PipelineBindPoint::COMPUTE,
                    pipeline_ref.get_pipeline_layout(),
                    0,
                    &vec![],
                    &[]);
            }
        };
    }

    // execute node
    node.execute(
        device,
        command_buffer);
}

#[tracing::instrument(skip(renderpass_manager, pipeline_manager, render_context, node))]
pub fn execute_graphics_node(
    renderpass_manager: &VulkanRenderpassManager,
    pipeline_manager: &Mutex<VulkanPipelineManager>,
    descriptor_sets: Arc<RwLock<Vec<vk::DescriptorSet>>>,
    descriptor_pool: vk::DescriptorPool,
    render_context: &VulkanRenderContext,
    command_buffer: &vk::CommandBuffer,
    node: &mut GraphicsPassNode) {

    let active_pipeline = &node.pipeline_description;
    if let Some(pipeline_description) = active_pipeline {
        // resolve render targets for this node
        let resolved_render_targets = {
            let render_targets = &node.render_targets;
            resolve_render_targets(render_targets)
        };

        let resolved_depth_target = {
            if let Some(depth_target) = &node.depth_target {
                // Some(resolve_render_targets(std::slice::from_ref(depth_target))[0].clone())
                resolve_render_targets(std::slice::from_ref(depth_target)).pop()
            } else {
                None
            }
        };

        // Ensure all rendertargets are the same dimensions
        let framebuffer_extent = {
            let mut extent: Option<vk::Extent3D> = None;
            for rt in &resolved_render_targets {
                match extent {
                    Some(extent) => {
                        assert_eq!(extent, rt.extent, "All framebuffer attachments must be the same dimensions");
                    },
                    None => {
                        extent = Some(rt.extent.clone());
                    }
                }
            }
            extent.expect("Framebuffer required for renderpass")
        };


        {
            let renderpass = renderpass_manager.create_or_fetch_renderpass(
                node.get_name(),
                &node.render_targets,
                &node.depth_target,
                render_context.get_device());
            let renderpass_ref = renderpass.lock().unwrap();

            let pipeline = pipeline_manager.lock().unwrap().create_pipeline(
                render_context.get_device(),
                renderpass_ref.renderpass.clone(),
                pipeline_description);
            let pipeline_ref = pipeline.lock().unwrap();

            let mut new_descriptor_sets = render_context.create_descriptor_sets(
                &pipeline_ref.device_pipeline.descriptor_set_layouts, descriptor_pool);

            // create framebuffer
            // TODO: should cache framebuffer objects to avoid creating the same ones each frame
            let framebuffer = {
                let framebuffer = render_context.create_framebuffer(
                    renderpass_ref.renderpass.clone(),
                    &framebuffer_extent,
                    &resolved_render_targets,
                    &resolved_depth_target);
                // Framebuffer needs to be owned by the GraphicsPassNode to ensure it's
                // destroyed after this frame has rendered
                node.framebuffer = Some(framebuffer);
                node.get_framebuffer()
            };

            // TODO: parameterize this per framebuffer attachment
            let clear_value = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.1, 0.1, 0.1, 1.0]
                }
            };

            // prepare and perform descriptor writes
            {
                let mut descriptor_updates = DescriptorUpdate::new();

                // get input and output handles for this pass
                // let inputs = node.get_inputs();
                let inputs = &node.inputs;
                let outputs = node.get_outputs();

                resolve_descriptors(
                    inputs,
                    pipeline_ref.deref(),
                    &new_descriptor_sets,
                    &mut descriptor_updates);
                resolve_descriptors(
                    outputs,
                    pipeline_ref.deref(),
                    &new_descriptor_sets,
                    &mut descriptor_updates);

                let descriptor_writes = descriptor_updates.create_descriptor_writes();

                unsafe {
                    enter_span!(tracing::Level::TRACE, "Update and bind descriptor sets");
                    let device = render_context.get_device();
                    // TODO: support descriptor copies?
                    device.get().update_descriptor_sets(
                        &descriptor_writes,
                        &[]);
                    // bind descriptorsets
                    // TODO: COMPUTE SUPPORT
                    device.get().cmd_bind_descriptor_sets(
                        *command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_ref.get_pipeline_layout(),
                        0,
                        &new_descriptor_sets,
                        &[]);
                }
            }

            // begin render pass and bind pipeline
            {
                let render_pass_begin = vk::RenderPassBeginInfo::default()
                    .render_pass(renderpass_ref.renderpass.clone())
                    .framebuffer(framebuffer)
                    .render_area(vk::Rect2D::default()
                        .offset(vk::Offset2D{x: 0, y: 0})
                        .extent(vk::Extent2D{
                            width: framebuffer_extent.width,
                            height: framebuffer_extent.height}))
                    .clear_values(std::slice::from_ref(&clear_value));

                unsafe {
                    let device = render_context.get_device();
                    enter_span!(tracing::Level::TRACE, "Begin renderpass & bind pipeline");
                    device.get().cmd_begin_render_pass(
                        *command_buffer,
                        &render_pass_begin,
                        vk::SubpassContents::INLINE);

                    // TODO: add compute support
                    device.get().cmd_bind_pipeline(
                        *command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_ref.get_pipeline());
                }
            }

            {
                descriptor_sets.write().unwrap().append(&mut new_descriptor_sets);
            }
        }

    }

    {
        let device = render_context.get_device();
        if let Some(viewport) = &node.viewport {
            unsafe {
                device.get().cmd_set_viewport(
                    *command_buffer,
                    0,
                    std::slice::from_ref(viewport));
            }
        }

        if let Some(scissor) = &node.scissor {
            unsafe {
                device.get().cmd_set_scissor(
                    *command_buffer,
                    0,
                    std::slice::from_ref(scissor));
            }
        }

        // execute this node
        node.execute(device, *command_buffer);
    }


    // if we began a render pass and bound a pipeline for this node, end it
    if active_pipeline.is_some() {
        unsafe {
            render_context.get_device().get().cmd_end_render_pass(*command_buffer);
        }
    }
}
