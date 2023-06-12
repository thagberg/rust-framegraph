extern crate petgraph;

use petgraph::stable_graph::{NodeIndex, StableDiGraph};
extern crate multimap;
use multimap::MultiMap;

extern crate context;
use context::render_context::{RenderContext};

use ash::vk;
use crate::frame::Frame;
use crate::frame_graph::FrameGraph;
use crate::pass_node::PassNode;
use crate::binding::{ResourceBinding, ImageBindingInfo, BufferBindingInfo, BindingType};
use crate::graphics_pass_node::{GraphicsPassNode};
use crate::pipeline::{Pipeline, VulkanPipelineManager};
use crate::renderpass_manager::VulkanRenderpassManager;

use std::collections::HashMap;
use std::ops::Deref;
use ash::vk::DeviceSize;
use petgraph::data::DataMap;
use petgraph::visit::Dfs;
use context::api_types::buffer::BufferWrapper;
use context::api_types::device::ResourceType;
use context::api_types::image::ImageWrapper;
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::barrier::{BufferBarrier, ImageBarrier};
use crate::command_list::CommandList;
use crate::compute_pass_node::ComputePassNode;
use crate::copy_pass_node::CopyPassNode;
use crate::pass_type::PassType;

#[derive(Clone)]
struct ResourceUsage {
    access: vk::AccessFlags,
    stage: vk::PipelineStageFlags,
    layout: Option<vk::ImageLayout>
}

fn is_write(access: vk::AccessFlags, stage: vk::PipelineStageFlags) -> bool {
    let write_access=
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE |
            vk::AccessFlags::SHADER_WRITE |
            vk::AccessFlags::TRANSFER_WRITE |
            vk::AccessFlags::HOST_WRITE |
            vk::AccessFlags::MEMORY_WRITE;

    let pipeline_write = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;

    (write_access & access != vk::AccessFlags::NONE) || (pipeline_write & stage != vk::PipelineStageFlags::NONE)
}

fn link_inputs(inputs: &[ResourceBinding], node_barrier: &mut NodeBarriers, usage_cache: &mut HashMap<u64, ResourceUsage>) {
    for input in inputs {
        let handle = input.resource.borrow().get_handle();

        let mut resource = input.resource.borrow_mut();
        let resolved_resource = {
            match &mut resource.resource_type {
                None => {
                    panic!("Invalid input binding")
                }
                Some(resource) => {
                    resource
                }
            }
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
                                // layout: Some(vk::ImageLayout::UNDEFINED)
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

fn get_descriptor_image_info(
    image: &ImageWrapper,
    binding_info: &ImageBindingInfo) -> (vk::DescriptorImageInfo, vk::DescriptorType) {

    let (sampler, descriptor_type) = match image.sampler {
        Some(s) => {(s, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)}
        // None => {(vk::Sampler::null(), vk::DescriptorType::SAMPLED_IMAGE)}
        None => {(vk::Sampler::null(), vk::DescriptorType::STORAGE_IMAGE)}
    };
    let image_info = vk::DescriptorImageInfo::builder()
        .image_view(image.view)
        .image_layout(binding_info.layout)
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

/// Wrapper for all info required for vk::WriteDescriptorSet
/// This ensures that the image / buffer info references held in WriteDescriptorSet
/// will live long enough
struct DescriptorUpdate {
    descriptor_writes: Vec<vk::WriteDescriptorSet>,
    image_infos: Vec<vk::DescriptorImageInfo>,
    buffer_infos: Vec<vk::DescriptorBufferInfo>
}

impl DescriptorUpdate {
    pub fn new() -> Self {
        DescriptorUpdate {
            descriptor_writes: vec![],
            image_infos: vec![],
            buffer_infos: vec![]
        }
    }
}

fn resolve_descriptors<'a, 'b>(
    bindings: &[ResourceBinding],
    pipeline: &Pipeline,
    descriptor_updates: &mut DescriptorUpdate) {

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
                let (image_info, descriptor_type) = get_descriptor_image_info(resolved_image, image_binding);
                descriptor_updates.image_infos.push(image_info);
                descriptor_write_builder = descriptor_write_builder
                    .descriptor_type(descriptor_type)
                    .image_info(std::slice::from_ref(descriptor_updates.image_infos.last().unwrap()));
            },
            (ResourceType::Buffer(resolved_buffer), BindingType::Buffer(buffer_binding)) => {
                let (buffer_info, descriptor_type) = get_descriptor_buffer_info(resolved_buffer, buffer_binding);
                descriptor_updates.buffer_infos.push(buffer_info);
                descriptor_write_builder = descriptor_write_builder
                    .descriptor_type(descriptor_type)
                    .buffer_info(std::slice::from_ref(descriptor_updates.buffer_infos.last().unwrap()));
            },
            _ => {
                panic!("Invalid type being resolved");
            }
        }

        descriptor_updates.descriptor_writes.push(descriptor_write_builder.build());
    }
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

    fn compile(&mut self, nodes: &mut StableDiGraph<PassType, u32>, root_index: NodeIndex) -> Vec<NodeIndex>{
        // create input/output maps to detect graph edges
        let mut input_map = MultiMap::new();
        let mut output_map = MultiMap::new();
        for node_index in nodes.node_indices() {
            let node = &nodes[node_index];
            for read in node.get_reads() {
                input_map.insert(read, node_index);
            }
            for write in node.get_writes() {
                output_map.insert(write, node_index);
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
            let sort_result = petgraph::algo::toposort(&*nodes, None);
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
                    panic!("A cycle was detected in the framegraph: {:?}", cycle_error);
                }
            }
        }

        // with sorted execution order we now want to look for opportunities to break the full set
        // of nodes into multiple command lists
        // This is required when execution requires synchronization across queues (e.g. going from
        // graphics to compute) or between CPU and GPU
        let mut command_lists: Vec<CommandList<NodeIndex>> = Vec::new();
        for i in &sorted_nodes {
            if let Some(node) = nodes.node_weight(*i) {

            }
        }

        sorted_nodes
    }

    fn link(
        &mut self,
        nodes: &mut StableDiGraph<PassType, u32>,
        sorted_nodes: &Vec<NodeIndex>) {

        // All image bindings and attachments require the most recent usage for that resource
        // in case layout transitions are necessary. Since the graph has already been sorted,
        // we can just iterate over the sorted nodes to do this
        let mut usage_cache: HashMap<u64, ResourceUsage> = HashMap::new();
        for node_index in sorted_nodes {
            if let Some(node) = nodes.node_weight_mut(*node_index) {
                let mut node_barrier = NodeBarriers {
                    image_barriers: vec![],
                    buffer_barriers: vec![]
                };

                match node {
                    PassType::Graphics(gn) => {
                        link_inputs(gn.get_inputs(), &mut node_barrier, &mut usage_cache);

                        for rt in gn.get_rendertargets_mut() {
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
                    }
                    PassType::Copy(cn) => {
                        for resource in &cn.copy_sources {
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

                        for resource in &cn.copy_dests {
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
                    },
                    PassType::Compute(cn) => {
                        link_inputs(&cn.inputs, &mut node_barrier, &mut usage_cache);
                        link_inputs(&cn.outputs, &mut node_barrier, &mut usage_cache);
                    }
                }

                self.node_barriers.insert(*node_index, node_barrier);
            }
        }

        //usage_cache
    }

    /// The purpose of finalize is to either generate new "finalized" nodes or mutate the existing
    /// nodes to add framebuffer and renderpass. This is to ensure that framebuffer objects are getting
    /// deleted after a frame completes rendering
    fn finalize(
        &mut self,
        nodes: &StableDiGraph<GraphicsPassNode, u32>,
        sorted_nodes: &Vec<NodeIndex>) {

        for index in sorted_nodes {
            let _node = nodes.node_weight(*index).unwrap();

            // generate renderpass

            // generate framebuffer

            // add to passnode?
        }
    }

    fn execute_copy_node(
        &mut self,
        render_context: &mut VulkanRenderContext,
        command_buffer: &vk::CommandBuffer,
        node: &mut CopyPassNode) {

        // Copy node is ez-pz
        node.execute(
            render_context,
            command_buffer);
    }

    fn execute_compute_node(
        &mut self,
        render_context: &mut VulkanRenderContext,
        command_buffer: &vk::CommandBuffer,
        node: &mut ComputePassNode) {

        // get compute pipeline from node's pipeline description
        let pipeline = self.pipeline_manager.create_compute_pipeline(
            render_context,
            &node.pipeline_description);

        // prepare and perform descriptor writes
        {
            let mut descriptor_updates = DescriptorUpdate::new();

            // get input and output handles for this pass
            // let inputs = node.get_inputs();
            let inputs = &node.inputs;
            let outputs = &node.outputs;

            resolve_descriptors(
                inputs,
                pipeline.borrow().deref(),
                &mut descriptor_updates);
            resolve_descriptors(
                outputs,
                pipeline.borrow().deref(),
                &mut descriptor_updates);

            unsafe {
                // TODO: support descriptor copies?
                render_context.get_device().borrow().get().update_descriptor_sets(
                    &descriptor_updates.descriptor_writes,
                    &[]);
                // bind descriptorsets
                // TODO: COMPUTE SUPPORT
                render_context.get_device().borrow().get().cmd_bind_descriptor_sets(
                    *command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline.borrow().get_pipeline_layout(),
                    0,
                    &pipeline.borrow().descriptor_sets,
                    &[]);
            }
        };

        // bind pipeline
        unsafe {
            render_context.get_device().borrow().get().cmd_bind_pipeline(
                *command_buffer,
                vk::PipelineBindPoint::COMPUTE,
                pipeline.borrow().get_pipeline());
        }

        // execute node
        node.execute(
            render_context,
            command_buffer);
    }

    fn execute_graphics_node(
        &mut self,
        render_context: &mut VulkanRenderContext,
        command_buffer: &vk::CommandBuffer,
        node: &mut GraphicsPassNode) {

        let active_pipeline = &node.pipeline_description;
        if let Some(pipeline_description) = active_pipeline {
            // resolve render targets for this node
            let resolved_render_targets = {
                let render_targets = &node.render_targets;
                resolve_render_targets(render_targets)
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

            let renderpass = self.renderpass_manager.create_or_fetch_renderpass(
                node.get_name(),
                &node.render_targets,
                render_context.get_device());

            let pipeline = self.pipeline_manager.create_pipeline(render_context, renderpass.borrow().renderpass.clone(), pipeline_description);

            // create framebuffer
            // TODO: should cache framebuffer objects to avoid creating the same ones each frame
            let framebuffer = {
                let framebuffer = render_context.create_framebuffer(
                    renderpass.borrow().renderpass.clone(),
                    &framebuffer_extent,
                    &resolved_render_targets);
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
                    pipeline.borrow().deref(),
                    &mut descriptor_updates);
                resolve_descriptors(
                    outputs,
                    pipeline.borrow().deref(),
                    &mut descriptor_updates);

                unsafe {
                    // TODO: support descriptor copies?
                    render_context.get_device().borrow().get().update_descriptor_sets(
                        &descriptor_updates.descriptor_writes,
                        &[]);
                    // bind descriptorsets
                    // TODO: COMPUTE SUPPORT
                    render_context.get_device().borrow().get().cmd_bind_descriptor_sets(
                        *command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline.borrow().get_pipeline_layout(),
                        0,
                        &pipeline.borrow().descriptor_sets,
                        &[]);
                }
            };

            // begin render pass and bind pipeline
            {
                let render_pass_begin = vk::RenderPassBeginInfo::builder()
                    .render_pass(renderpass.borrow().renderpass.clone())
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
                        pipeline.borrow().get_pipeline());
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

}

impl FrameGraph for VulkanFrameGraph {
    type PN = GraphicsPassNode;
    type RPM = VulkanRenderpassManager;
    type PM = VulkanPipelineManager;
    type CB = vk::CommandBuffer;
    type RC = VulkanRenderContext;
    type Index = NodeIndex;

    fn start(&mut self) -> Box<Frame> {
        Box::new(Frame::new())
    }

    fn end(
        &mut self,
        frame: &mut Frame,
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
        let sorted_nodes = &frame.sorted_nodes;
        for index in sorted_nodes {
            let node = frame.nodes.node_weight_mut(*index).unwrap();

            // Prepare and execute resource barriers
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

            // prepare pipeline for execution (node's fill callback)
            match node {
                PassType::Graphics(graphics_node) => {
                    self.execute_graphics_node(render_context, command_buffer, graphics_node);
                },
                PassType::Copy(copy_node) => {
                    self.execute_copy_node(render_context, command_buffer, copy_node);
                },
                PassType::Compute(compute_node) => {
                    self.execute_compute_node(render_context, command_buffer, compute_node);
                }
                _ => {}
            }
        }
    }
}