extern crate petgraph;

use std::cell::RefCell;
use petgraph::{graph, stable_graph, Direction, Directed};
use petgraph::stable_graph::{Edges, NodeIndex, StableDiGraph};
extern crate multimap;
use multimap::MultiMap;

extern crate context;
use context::render_context::{RenderContext, CommandBuffer};

use ash::vk;
use crate::frame::Frame;
use crate::frame_graph::FrameGraph;
use crate::pass_node::PassNode;
use crate::binding::{ResourceBinding, ResolvedResourceBinding, BindingInfo, ImageBindingInfo, BufferBindingInfo, BindingType};
use crate::pass_node::ResolvedBindingMap;
use crate::graphics_pass_node::{GraphicsPassNode};
use crate::pipeline::{Pipeline, PipelineManager, VulkanPipelineManager};
use crate::resource::resource_manager::ResourceManager;
use crate::resource::vulkan_resource_manager::{ResolvedResource, ResolvedResourceMap, ResourceCreateInfo, ResourceHandle, VulkanResourceManager};
use crate::renderpass_manager::{RenderpassManager, VulkanRenderpassManager, AttachmentInfo, StencilAttachmentInfo};

use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use ash::vk::{BufferMemoryBarrier, DeviceSize, Sampler};
use petgraph::adj::DefaultIx;
use petgraph::data::DataMap;
use petgraph::visit::{Dfs, EdgeRef, NodeCount};
use context::api_types::buffer::BufferWrapper;
use context::api_types::device::{DeviceResource, ResourceType};
use context::api_types::image::ImageWrapper;
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::barrier::{BufferBarrier, ImageBarrier};

fn resolve_copy_resources(
    resource_manager: &VulkanResourceManager,
    handles: &[ResourceHandle]) -> ResolvedResourceMap {

    let mut resolved_map = ResolvedResourceMap::new();

    for handle in handles {
        let resolved = resource_manager.resolve_resource(handle);
        resolved_map.insert(*handle, resolved.clone());
    }

    resolved_map
}

fn resolve_render_targets(
    attachments: &[AttachmentReference]) -> Vec<ImageWrapper> {

    let mut rts: Vec<ImageWrapper> = Vec::new();
    for attachment in attachments {
        let attachment_image = attachment.resource_image.borrow();
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

fn get_descriptor_image_info(image: &ImageWrapper) -> (vk::DescriptorImageInfo, vk::DescriptorType) {
    let (sampler, descriptor_type) = match image.sampler {
        Some(s) => {(s, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)}
        None => {(vk::Sampler::null(), vk::DescriptorType::SAMPLED_IMAGE)}
    };
    let image_info = vk::DescriptorImageInfo::builder()
        .image_view(image.view)
        .image_layout(image.layout)
        .sampler(sampler)
        .build();

    (image_info, descriptor_type)
}

fn get_descriptor_buffer_info(
    buffer: &BufferWrapper,
    binding: &BufferBindingInfo) -> (vk::DescriptorBufferInfo, vk::DescriptorType) {

    let buffer_info = vk::DescriptorBufferInfo::builder()
        .buffer(buffer.buffer)
        .offset(binding.offset)
        .range(binding.range)
        .build();
    let descriptor_type = vk::DescriptorType::UNIFORM_BUFFER; // TODO: this could also be a storage buffer

    (buffer_info, descriptor_type)
}

fn resolve_resources_and_descriptors(
    bindings: &[ResourceBinding],
    pipeline: &mut Pipeline,
    image_bindings: &mut Vec<vk::DescriptorImageInfo>,
    buffer_bindings: &mut Vec<vk::DescriptorBufferInfo>,
    descriptor_writes: &mut Vec<vk::WriteDescriptorSet>) -> ResolvedBindingMap {

    let mut resolved_map = ResolvedBindingMap::new();

    for binding in bindings {
        let binding_ref = binding.resource.borrow();
        let resolved_binding = binding_ref.resource_type.as_ref().expect("Invalid resource in binding");
        let descriptor_set = pipeline.descriptor_sets[binding.binding_info.set as usize];

        let mut descriptor_write_builder = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(binding.binding_info.slot)
            .dst_array_element(0); // TODO: parameterize

        match (&resolved_binding, &binding.binding_info.binding_type) {
            (ResourceType::Image(resolved_image), BindingType::Image(image_binding)) => {
                let (image_info, descriptor_type) = get_descriptor_image_info(resolved_image);
                image_bindings.push(image_info);
                descriptor_write_builder = descriptor_write_builder
                    .descriptor_type(descriptor_type)
                    .image_info(std::slice::from_ref(image_bindings.last().unwrap()));
            },
            (ResourceType::Buffer(resolved_buffer), BindingType::Buffer(buffer_binding)) => {
                let (buffer_info, descriptor_type) = get_descriptor_buffer_info(resolved_buffer, buffer_binding);
                buffer_bindings.push(buffer_info);
                descriptor_write_builder = descriptor_write_builder
                    .descriptor_type(descriptor_type)
                    .buffer_info(std::slice::from_ref(buffer_bindings.last().unwrap()));
            },
            _ => {
                panic!("Invalid type being resolved");
            }
        }

        descriptor_writes.push(descriptor_write_builder.build());
    }

    resolved_map
}

pub struct NodeBarriers {
    image_barriers: Vec<ImageBarrier>,
    buffer_barriers: Vec<BufferBarrier>
}

pub struct VulkanFrameGraph {
    pipeline_manager: VulkanPipelineManager,
    renderpass_manager: VulkanRenderpassManager,
    node_barriers: HashMap<NodeIndex, NodeBarriers>
}

impl VulkanFrameGraph {
    pub fn new(
        renderpass_manager: VulkanRenderpassManager,
        pipeline_manager: VulkanPipelineManager) -> VulkanFrameGraph {

        VulkanFrameGraph {
            pipeline_manager,
            renderpass_manager,
            node_barriers: HashMap::new()
        }
    }

    fn compile(&mut self, nodes: &mut StableDiGraph<GraphicsPassNode, u32>, root_index: NodeIndex) -> Vec<NodeIndex>{
        // create input/output maps to detect graph edges
        let mut input_map = MultiMap::new();
        let mut output_map = MultiMap::new();
        for node_index in nodes.node_indices() {
            let node = &nodes[node_index];
            for input in node.get_inputs() {
                input_map.insert(input.resource.borrow().get_handle(), node_index);
            }
            for copy_source in node.get_copy_sources() {
                input_map.insert(copy_source.borrow().get_handle(), node_index);
            }

            for output in node.get_outputs() {
                output_map.insert(output.resource.borrow().get_handle(), node_index);
            }
            for rt in node.get_rendertargets() {
                output_map.insert(rt.resource_image.borrow().get_handle(), node_index);
            }
            for copy_dest in node.get_copy_dests() {
                output_map.insert(copy_dest.borrow().get_handle(), node_index);
            }
        }

        // iterate over input map. For each input, find matching outputs and then
        // generate a graph edge for each pairing
        for (input, node_index) in input_map.iter() {
            let find_outputs = output_map.get_vec(input);
            if let Some(matched_outputs) = find_outputs {
                // input/output match defines a graph edge
                for matched_output in matched_outputs {
                    // use update_edge instead of add_edge to avoid duplicates
                    nodes.update_edge(
                        *node_index,
                        *matched_output,
                        0);
                }
            }
        }

        // Use DFS to find all accessible nodes from the root node
        {
            let mut retained_nodes: Vec<bool> = Vec::new();
            retained_nodes.resize(nodes.node_count(), false);

            //let mut dfs = Dfs::new(&nodes, root_index);
            let mut dfs = Dfs::new(&*nodes, root_index);
            while let Some(node_id) = dfs.next(&*nodes) {
                retained_nodes[node_id.index()] = true;
            }

            nodes.retain_nodes(|_graph, node_index| {
                retained_nodes[node_index.index()]
            });
        }

        // unresolved and unused passes have been removed from the graph,
        // so now we can use a topological sort to generate an execution order
        let mut sorted_nodes: Vec<NodeIndex> = Vec::new();
        {
            let mut sort_result = petgraph::algo::toposort(&*nodes, None);
            match sort_result {
                Ok(mut sorted_list) => {
                    // DFS requires we order nodes as input -> output, but for sorting we want output -> input
                    sorted_list.reverse();
                    for i in &sorted_list {
                        println!("Node: {:?}", nodes.node_weight(*i).unwrap().get_name());
                    }
                    sorted_nodes = sorted_list;
                },
                Err(cycle_error) => {
                    println!("A cycle was detected in the framegraph: {:?}", cycle_error);
                }
            }
        }

        sorted_nodes
    }

    fn link(
        &mut self,
        nodes: &mut StableDiGraph<GraphicsPassNode, u32>,
        sorted_nodes: &Vec<NodeIndex>) {

        // All image bindings and attachments require the most recent usage for that resource
        // in case layout transitions are necessary. Since the graph has already been sorted,
        // we can just iterate over the sorted nodes to do this
        #[derive(Clone)]
        struct ResourceUsage {
            access: vk::AccessFlags,
            stage: vk::PipelineStageFlags,
            layout: Option<vk::ImageLayout>
        }
        let mut usage_cache: HashMap<u64, ResourceUsage> = HashMap::new();
        for node_index in sorted_nodes {
            if let Some(node) = nodes.node_weight_mut(*node_index) {
                let is_write = |access: vk::AccessFlags, stage: vk::PipelineStageFlags|-> bool {
                    let write_access=
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE |
                        vk::AccessFlags::SHADER_WRITE |
                        vk::AccessFlags::TRANSFER_WRITE |
                        vk::AccessFlags::HOST_WRITE |
                        vk::AccessFlags::MEMORY_WRITE;

                    let pipeline_write = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;

                    (write_access & access != vk::AccessFlags::NONE) || (pipeline_write & stage != vk::PipelineStageFlags::NONE)
                };

                let mut node_barrier = NodeBarriers {
                    image_barriers: vec![],
                    buffer_barriers: vec![]
                };

                for rt in node.get_rendertargets_mut() {
                    // rendertargets always write, so if this isn't the first usage of this resource
                    // then we know we need a barrier
                    let handle = rt.resource_image.borrow().get_handle();
                    let last_usage = usage_cache.get(&handle);
                    let new_usage = ResourceUsage {
                        access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        // layout: Some(rt.layout)
                        layout: Some(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    };
                    if let Some(usage) = last_usage {
                        rt.layout = usage.layout.expect("Tried to get image layout from non-image");

                        let image_barrier = ImageBarrier {
                            resource: rt.resource_image.clone(),
                            source_stage: usage.stage,
                            dest_stage: new_usage.stage,
                            source_access: usage.access,
                            dest_access: new_usage.access,
                            old_layout: rt.layout,
                            new_layout: new_usage.layout.unwrap()
                        };
                        node_barrier.image_barriers.push(image_barrier);
                    }

                    usage_cache.insert(handle, new_usage);
                }

                for resource in node.get_copy_sources() {
                    let handle = resource.borrow().get_handle();
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

                for resource in node.get_copy_dests() {
                    let handle = resource.borrow().get_handle();
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

                for input in node.get_inputs() {
                    //let last_usage = usage_cache.get(&input.handle);
                    let handle = input.resource.borrow().get_handle();
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
                        }

                        usage_cache.insert(handle, new_usage);
                        //image_binding.layout = update_usage(input.handle, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    } else {
                        //panic!("Buffer barriers not implemented");
                    }
                }

                self.node_barriers.insert(*node_index, node_barrier);
            }
        }

        //usage_cache
    }
}

impl FrameGraph for VulkanFrameGraph {
    type PN = GraphicsPassNode;
    type RPM = VulkanRenderpassManager;
    type PM = VulkanPipelineManager;
    type CB = vk::CommandBuffer;
    type RC = VulkanRenderContext;
    type Index = NodeIndex;

    fn start(&mut self) -> Frame {
        Frame::new()
    }

    fn end(
        &mut self,
        mut frame: Frame,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB) {

        frame.end();

        let root_index = frame.get_root_index();

        // compile and link frame
        {
            let sorted_nodes = self.compile(&mut frame.nodes, root_index);
            self.link(&mut frame.nodes, &sorted_nodes);
            //frame.set_sorted_nodes(sorted_nodes);
            frame.sorted_nodes = sorted_nodes;
        }

        // excute nodes
        for index in frame.get_sorted_nodes() {
            let node = frame.nodes.node_weight(*index).unwrap();

            // get input and output handles for this pass
            let inputs = node.get_inputs();
            let outputs = node.get_outputs();
            let render_targets = node.get_rendertargets();
            let copy_sources = node.get_copy_sources();
            let copy_dests = node.get_copy_dests();

            // resolve copy sources and dests for this node
            // let resolved_copy_sources = resolve_copy_resources(resource_manager, copy_sources);
            // let resolved_copy_dests = resolve_copy_resources(resource_manager, copy_dests);

            // prepare pipeline for execution (node's fill callback)
            let active_pipeline = node.get_pipeline_description();
            if let Some(pipeline_description) = active_pipeline {
                // resolve render targets for this node
                let resolved_render_targets = resolve_render_targets(render_targets);

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

                let renderpass = self.renderpass_manager.create_or_fetch_renderpass(
                    node,
                    node.get_rendertargets(),
                    render_context);

                let mut pipeline = self.pipeline_manager.create_pipeline(render_context, renderpass, pipeline_description);

                // create framebuffer
                // TODO: should cache framebuffer objects to avoid creating the same ones each frame
                let framebuffer = render_context.create_framebuffer(
                    renderpass,
                    &framebuffer_extent,
                    &resolved_render_targets);

                // TODO: parameterize this per framebuffer attachment
                let clear_value = vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.1, 0.1, 0.1, 1.0]
                    }
                };

                // TODO: don't need to resolve these anymore, just handle descriptor sets
                // prepare and perform descriptor writes
                let (resolved_inputs, resolved_outputs) = {
                    let mut descriptor_writes: Vec<vk::WriteDescriptorSet> = Vec::new();
                    let mut image_bindings: Vec<vk::DescriptorImageInfo> = Vec::new();
                    let mut buffer_bindings: Vec<vk::DescriptorBufferInfo> = Vec::new();

                    let resolved_inputs = resolve_resources_and_descriptors(
                        inputs,
                        &mut pipeline,
                        &mut image_bindings,
                        &mut buffer_bindings,
                        &mut descriptor_writes);
                    let resolved_outputs = resolve_resources_and_descriptors(
                        outputs,
                        &mut pipeline,
                        &mut image_bindings,
                        &mut buffer_bindings,
                        &mut descriptor_writes);

                    unsafe {
                        // TODO: support descriptor copies?
                        render_context.get_device().borrow().get().update_descriptor_sets(
                            &descriptor_writes,
                            &[]);
                        // bind descriptorsets
                        // TODO: COMPUTE SUPPORT
                        render_context.get_device().borrow().get().cmd_bind_descriptor_sets(
                            *command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline.pipeline_layout,
                            0,
                            &pipeline.descriptor_sets,
                            &[]);
                    }

                    (resolved_inputs, resolved_outputs)
                };

                // begin render pass and bind pipeline
                {
                    let render_pass_begin = vk::RenderPassBeginInfo::builder()
                        .render_pass(renderpass)
                        .framebuffer(framebuffer)
                        .render_area(vk::Rect2D::builder()
                            .offset(vk::Offset2D{x: 0, y: 0})
                            .extent(vk::Extent2D{
                                width: framebuffer_extent.width,
                                height: framebuffer_extent.height})
                            .build())
                        .clear_values(std::slice::from_ref(&clear_value));

                    unsafe {
                        render_context.get_device().borrow().get().cmd_begin_render_pass(
                            *command_buffer,
                            &render_pass_begin,
                            vk::SubpassContents::INLINE);

                        // TODO: add compute support
                        render_context.get_device().borrow().get().cmd_bind_pipeline(
                            *command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline.graphics_pipeline);
                    }
                }
            }

            let barriers = self.node_barriers.get(index);
            if let Some(barriers) = barriers {
                // Create the source and dest stage masks
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
                    let buffer = bb.resource.borrow();
                    let resolved = buffer.resource_type.as_ref().expect("Invalid buffer in BufferBarrier");
                    if let ResourceType::Buffer(resolved_buffer) = resolved {
                        vk::BufferMemoryBarrier::builder()
                            .buffer(resolved_buffer.buffer)
                            .src_access_mask(bb.source_access)
                            .dst_access_mask(bb.dest_access)
                            .offset(bb.offset as DeviceSize)
                            .size(bb.size as DeviceSize)
                            .src_queue_family_index(render_context.get_graphics_queue_index())
                            .dst_queue_family_index(render_context.get_graphics_queue_index())
                            .build()
                    } else {
                        panic!("Non buffer resource in BufferBarrier")
                    }
                }).collect();

                // translate from our ImageBarrier to Vulkan
                let transformed_image_barriers: Vec<vk::ImageMemoryBarrier> = barriers.image_barriers.iter().map(|ib| {
                    let image = ib.resource.borrow();
                    let resolved = image.resource_type.as_ref().expect("Invalid image in ImageBarrier");
                    // TODO: the range needs to be parameterized
                    let range = vk::ImageSubresourceRange::builder()
                        .level_count(1)
                        .base_mip_level(0)
                        .layer_count(1)
                        .base_array_layer(0)
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .build();
                    if let ResourceType::Image(resolved_image) = resolved {
                        vk::ImageMemoryBarrier::builder()
                            .image(resolved_image.image)
                            .src_access_mask(ib.source_access)
                            .dst_access_mask(ib.dest_access)
                            .old_layout(ib.old_layout)
                            .new_layout(ib.new_layout)
                            .src_queue_family_index(render_context.get_graphics_queue_index())
                            .dst_queue_family_index(render_context.get_graphics_queue_index())
                            .subresource_range(range)
                            .build()
                    } else {
                        panic!("Non image resource in ImageBarrier")
                    }
                }).collect();

                if transformed_image_barriers.len() > 0 || transformed_buffer_barriers.len() > 0 {
                    unsafe {
                        render_context.get_device().borrow().get().cmd_pipeline_barrier(
                            *command_buffer,
                            source_stage,
                            dest_stage,
                            vk::DependencyFlags::empty(),
                            &[],
                            &transformed_buffer_barriers,
                            &transformed_image_barriers);
                    }
                }
            }

            // execute this node
            node.execute(
                render_context,
                command_buffer);

            // if we began a render pass and bound a pipeline for this node, end it
            if active_pipeline.is_some() {
                unsafe {
                    render_context.get_device().borrow().get().cmd_end_render_pass(*command_buffer);
                }
            }
        }

        // Free transient resources
        // TODO: Does this need to wait until GPU execution is finished?
    }
}