use std::collections::HashMap;
use ash::vk;
use api_types::device::resource::ResourceType;
use context::vulkan_render_context::VulkanRenderContext;
use crate::binding::{ResourceBinding, BindingType};
use crate::barrier::{ImageBarrier, BufferBarrier};
use crate::graphics_pass_node::GraphicsPassNode;
use crate::copy_pass_node::CopyPassNode;
use crate::compute_pass_node::ComputePassNode;
use crate::present_pass_node::PresentPassNode;

#[derive(Clone)]
pub struct ResourceUsage {
    pub access: vk::AccessFlags,
    pub stage: vk::PipelineStageFlags,
    pub layout: Option<vk::ImageLayout>
}

pub struct NodeBarriers {
    pub image_barriers: Vec<ImageBarrier>,
    pub buffer_barriers: Vec<BufferBarrier>
}

impl std::fmt::Debug for NodeBarriers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeBarriers")
            .field("image barriers", &self.image_barriers.len())
            .field("buffer barriers", &self.buffer_barriers.len())
            .finish()
    }
}

pub struct BarrierTranslation {
    pub source_stage: vk::PipelineStageFlags,
    pub dest_stage: vk::PipelineStageFlags,
    pub image_barriers: Vec<vk::ImageMemoryBarrier<'static>>,
    pub buffer_barriers: Vec<vk::BufferMemoryBarrier<'static>>
}

pub fn translate_barriers(
    barriers: &NodeBarriers,
    render_context: &VulkanRenderContext) -> BarrierTranslation {

    let mut source_stage = vk::PipelineStageFlags::NONE;
    let mut dest_stage = vk::PipelineStageFlags::NONE;
    for image_barrier in &barriers.image_barriers {
        source_stage |= image_barrier.source_stage;
        dest_stage |= image_barrier.dest_stage;
    }
    for buffer_barrier in &barriers.buffer_barriers {
        source_stage |= buffer_barrier.source_stage;
        dest_stage |= buffer_barrier.dest_stage;
    }

    // translate from our BufferBarrier to Vulkan
    let transformed_buffer_barriers: Vec<vk::BufferMemoryBarrier> = barriers.buffer_barriers.iter().map(|bb| {
        let buffer = bb.resource.lock().unwrap();
        let resolved = buffer.resource_type.as_ref().expect("Invalid buffer in BufferBarrier");
        if let ResourceType::Buffer(resolved_buffer) = resolved {
            vk::BufferMemoryBarrier::default()
                .buffer(resolved_buffer.buffer)
                .src_access_mask(bb.source_access)
                .dst_access_mask(bb.dest_access)
                .offset(bb.offset as vk::DeviceSize)
                .size(bb.size as vk::DeviceSize)
                .src_queue_family_index(render_context.get_graphics_queue_index())
                .dst_queue_family_index(render_context.get_graphics_queue_index())
        } else {
            panic!("Non buffer resource in BufferBarrier")
        }
    }).collect();

    // translate from our ImageBarrier to Vulkan
    let transformed_image_barriers: Vec<vk::ImageMemoryBarrier> = barriers.image_barriers.iter().map(|ib| {
        let image = ib.resource.lock().unwrap();
        let resolved = image.resource_type.as_ref().expect("Invalid image in ImageBarrier");
        if let ResourceType::Image(resolved_image) = resolved {
            let aspect_mask = util::image::get_aspect_mask_from_format(
                resolved_image.format);
            // TODO: the range needs to be parameterized
            let range = vk::ImageSubresourceRange::default()
                .level_count(1)
                .base_mip_level(0)
                .layer_count(1)
                .base_array_layer(0)
                .aspect_mask(aspect_mask);
            vk::ImageMemoryBarrier::default()
                .image(resolved_image.image)
                .src_access_mask(ib.source_access)
                .dst_access_mask(ib.dest_access)
                .old_layout(ib.old_layout)
                .new_layout(ib.new_layout)
                .src_queue_family_index(render_context.get_graphics_queue_index())
                .dst_queue_family_index(render_context.get_graphics_queue_index())
                .subresource_range(range)
        } else {
            panic!("Non image resource in ImageBarrier")
        }
    }).collect();

    BarrierTranslation {
        source_stage,
        dest_stage,
        image_barriers: transformed_image_barriers,
        buffer_barriers: transformed_buffer_barriers
    }
}

pub fn is_write(access: vk::AccessFlags, stage: vk::PipelineStageFlags) -> bool {
    let write_access=
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE |
            vk::AccessFlags::SHADER_WRITE |
            vk::AccessFlags::TRANSFER_WRITE |
            vk::AccessFlags::HOST_WRITE |
            vk::AccessFlags::MEMORY_WRITE;

    let pipeline_write = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;

    (write_access & access != vk::AccessFlags::NONE) || (pipeline_write & stage != vk::PipelineStageFlags::NONE)
}

pub fn link_inputs(inputs: &[ResourceBinding], node_barrier: &mut NodeBarriers, usage_cache: &mut HashMap<u64, ResourceUsage>) {
    for input in inputs {
        let mut input_ref = input.resource.lock().unwrap();
        let (handle, resolved_resource) = {

            let handle = input_ref.get_handle();

            let resolved_resource = {
                match &mut input_ref.resource_type {
                    None => {
                        panic!("Invalid input binding")
                    }
                    Some(resource) => {
                        resource
                    }
                }
            };

            (handle, resolved_resource)
        };

        match resolved_resource {
            ResourceType::Buffer(_) => {
                // Not implemented yet
            }
            ResourceType::Image(resolved_image) => {
                let last_usage = {
                    let usage = usage_cache.get(&handle);
                    match usage {
                        Some(found_usage) => {found_usage.clone()},
                        _ => {
                            ResourceUsage {
                                access: vk::AccessFlags::NONE,
                                stage: vk::PipelineStageFlags::ALL_COMMANDS,
                                layout: Some(resolved_image.layout)
                            }
                        }
                    }
                };

                // barrier required if:
                //  * last usage was a write
                //  * image layout has changed
                let prev_write = is_write(last_usage.access, last_usage.stage);

                if let BindingType::Image(image_binding) = &input.binding_info.binding_type {
                    let new_usage = ResourceUsage{
                        access: input.binding_info.access,
                        stage: input.binding_info.stage,
                        layout: Some(image_binding.layout)
                    };

                    let layout_changed = {
                        if let Some(layout) = last_usage.layout {
                            layout != image_binding.layout
                        } else {
                            true
                        }
                    };

                    // need a barrier
                    if layout_changed || prev_write {
                        let image_barrier = ImageBarrier {
                            resource: input.resource.clone(),
                            source_stage: last_usage.stage,
                            dest_stage: new_usage.stage,
                            source_access: last_usage.access,
                            dest_access: new_usage.access,
                            old_layout: last_usage.layout.expect("Using a non-image for an image transition"),
                            new_layout: new_usage.layout.unwrap()
                        };
                        node_barrier.image_barriers.push(image_barrier);
                        resolved_image.layout = new_usage.layout.unwrap();
                    }

                    usage_cache.insert(handle, new_usage);
                    //image_binding.layout = update_usage(input.handle, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                } else {
                    panic!("Buffer binding used on an image reosurce?");
                }
            }
        }

    }
}

pub fn link_graphics_node(node: &mut GraphicsPassNode, usage_cache: &mut HashMap<u64, ResourceUsage>) -> NodeBarriers {
    let mut node_barrier = NodeBarriers {
        image_barriers: vec![],
        buffer_barriers: vec![]
    };

    link_inputs(node.get_inputs(), &mut node_barrier, usage_cache);
    link_inputs(node.get_outputs(), &mut node_barrier, usage_cache);

    if let Some(dt) = node.get_depth_mut() {
        let handle = {
            dt.resource_image.lock().unwrap().get_handle()
        };
        let last_usage = usage_cache.get(&handle);
        // TODO: handle separate depth and stencil targets
        let new_usage = ResourceUsage {
            access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE |
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
            stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            layout: Some(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
        };
        if let Some(usage) = last_usage {
            // The RenderPassManager expects the RT layout to be in the
            // post-barrier (i.e. new) layout
            dt.layout = new_usage.layout.unwrap();

            let image_barrier = ImageBarrier {
                resource: dt.resource_image.clone(),
                source_stage: usage.stage,
                dest_stage: new_usage.stage,
                source_access: usage.access,
                dest_access: new_usage.access,
                old_layout: usage.layout.expect("Tried to get image layout from non-image"),
                new_layout: dt.layout
            };
            node_barrier.image_barriers.push(image_barrier);
        }
    }

    for rt in node.get_rendertargets_mut() {
        // rendertargets always write, so if this isn't the first usage of this resource
        // then we know we need a barrier
        let handle = {
            rt.resource_image.lock().unwrap().get_handle()
        };
        let last_usage = usage_cache.get(&handle);
        let new_usage = ResourceUsage {
            access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::COLOR_ATTACHMENT_READ,
            stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            layout: Some(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        };
        if let Some(usage) = last_usage {
            // The RenderPassManager expects the RT layout to be in the
            // post-barrier (i.e. new) layout
            rt.layout = new_usage.layout.unwrap();

            let image_barrier = ImageBarrier {
                resource: rt.resource_image.clone(),
                source_stage: usage.stage,
                dest_stage: new_usage.stage,
                source_access: usage.access,
                dest_access: new_usage.access,
                old_layout: usage.layout.expect("Tried to get image layout from non-image"),
                new_layout: rt.layout
            };
            node_barrier.image_barriers.push(image_barrier);
        }

        usage_cache.insert(handle, new_usage);
    }

    node_barrier
}

pub fn link_copy_node(node: &mut CopyPassNode, usage_cache: &mut HashMap<u64, ResourceUsage>) -> NodeBarriers {
    let mut node_barrier = NodeBarriers {
        image_barriers: vec![],
        buffer_barriers: vec![]
    };

    for resource in &node.copy_sources {
        let handle = {
            resource.lock().unwrap().get_handle()
        };
        let last_usage = {
            let usage = usage_cache.get(&handle);
            match usage {
                Some(found_usage) => {found_usage.clone()},
                _ => {
                    ResourceUsage {
                        access: vk::AccessFlags::NONE,
                        stage: vk::PipelineStageFlags::ALL_COMMANDS,
                        layout: Some(vk::ImageLayout::UNDEFINED)
                    }
                }
            }
        };

        let new_usage = ResourceUsage{
            access: vk::AccessFlags::TRANSFER_READ,
            stage: vk::PipelineStageFlags::TRANSFER,
            layout: Some(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        };

        // for copy sources and destinations, a barrier is always required
        let image_barrier = ImageBarrier {
            resource: resource.clone(),
            source_stage: last_usage.stage,
            dest_stage: new_usage.stage,
            source_access: last_usage.access,
            dest_access: new_usage.access,
            old_layout: last_usage.layout.expect("Using a non-image for an image transition"),
            new_layout: new_usage.layout.unwrap()
        };
        node_barrier.image_barriers.push(image_barrier);
    }

    for resource in &node.copy_dests {
        let handle = {
            resource.lock().unwrap().get_handle()
        };
        let last_usage = {
            let usage = usage_cache.get(&handle);
            match usage {
                Some(found_usage) => {found_usage.clone()},
                _ => {
                    ResourceUsage {
                        access: vk::AccessFlags::NONE,
                        stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                        layout: Some(vk::ImageLayout::UNDEFINED)
                    }
                }
            }
        };

        let new_usage = ResourceUsage{
            access: vk::AccessFlags::TRANSFER_WRITE,
            stage: vk::PipelineStageFlags::TRANSFER,
            layout: Some(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        };

        // for copy sources and destinations, a barrier is always required
        let image_barrier = ImageBarrier {
            resource: resource.clone(),
            source_stage: last_usage.stage,
            dest_stage: new_usage.stage,
            source_access: last_usage.access,
            dest_access: new_usage.access,
            old_layout: last_usage.layout.expect("Using a non-image for an image transition"),
            new_layout: new_usage.layout.unwrap()
        };
        node_barrier.image_barriers.push(image_barrier);
    }

    node_barrier
}

pub fn link_compute_node(node: &mut ComputePassNode, usage_cache: &mut HashMap<u64, ResourceUsage>) -> NodeBarriers {
    let mut node_barrier = NodeBarriers {
        image_barriers: vec![],
        buffer_barriers: vec![]
    };

    link_inputs(&node.inputs, &mut node_barrier, usage_cache);
    link_inputs(&node.outputs, &mut node_barrier, usage_cache);

    // TODO: implement the rest of this
    node_barrier
}

pub fn link_present_node(node: &mut PresentPassNode, usage_cache: &mut HashMap<u64, ResourceUsage>) -> NodeBarriers {
    let mut node_barrier = NodeBarriers {
        image_barriers: vec![],
        buffer_barriers: vec![]
    };

    // link_inputs(gn.get_inputs(), &mut node_barrier, &mut usage_cache);
    let mut swapchain = node.swapchain_image.lock().unwrap();
    let handle = swapchain.get_handle();
    let swapchain_image = swapchain.get_image_mut();
    let last_usage = {
        let usage = usage_cache.get(&handle);
        match usage {
            Some(found_usage) => {found_usage.clone()},
            _ => {
                ResourceUsage {
                    access: vk::AccessFlags::NONE,
                    stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                    layout: Some(swapchain_image.layout)
                }
            }
        }
    };

    let new_usage = ResourceUsage {
        access: vk::AccessFlags::NONE,
        stage: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        layout: Some(vk::ImageLayout::PRESENT_SRC_KHR),
    };

    let present_barrier = ImageBarrier {
        resource: node.swapchain_image.clone(),
        source_stage: last_usage.stage,
        dest_stage: new_usage.stage,
        source_access: last_usage.access,
        dest_access: new_usage.access,
        old_layout: last_usage.layout.expect("Using a non-image for an image transition"),
        new_layout: new_usage.layout.unwrap()
    };
    node_barrier.image_barriers.push(present_barrier);

    swapchain_image.layout = new_usage.layout.unwrap();
    usage_cache.insert(handle, new_usage);

    // command_lists.push(current_list);
    // current_list = CommandList::new();
    // current_list.wait = Some(QueueWait{
    //     // TODO: what was I doing here?
    //     wait_stage_mask: vk::PipelineStageFlags::NONE,
    // });

    node_barrier
}
