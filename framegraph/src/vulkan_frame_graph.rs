extern crate petgraph;
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
use crate::pipeline::{PipelineManager, VulkanPipelineManager};
use crate::resource::resource_manager::ResourceManager;
use crate::resource::vulkan_resource_manager::{ResolvedResource, ResolvedResourceMap, ResourceHandle, ResourceType, VulkanResourceManager};
use crate::renderpass_manager::{RenderpassManager, VulkanRenderpassManager, AttachmentInfo, StencilAttachmentInfo};

use std::collections::HashMap;
use std::marker::PhantomData;
use petgraph::adj::DefaultIx;
use petgraph::data::DataMap;
use petgraph::visit::{Dfs, EdgeRef, NodeCount};
use context::api_types::image::ImageWrapper;
use context::vulkan_render_context::VulkanRenderContext;
use crate::attachment::AttachmentReference;
use crate::barrier::ImageBarrier;

fn resolve_copy_resources(
    resource_manager: &VulkanResourceManager,
    handles: &[ResourceHandle]) -> ResolvedResourceMap {

    let mut resolved_map = ResolvedResourceMap::new()

    for handle in handles {
        let resolved = resource_manager.resolve_resoure(handle);
        resolved_map.insert(*handle, resolved.clone());
    }

    resolved_map
}

fn resolve_render_targets(
    resource_manager: &VulkanResourceManager,
    attachments: &[AttachmentReference]) -> Vec<ImageWrapper> {

    let mut rts: Vec<ImageWrapper> = Vec::new();
    for attachment in attachments {
      let resolved = resource_manager.resolve_resource(&attachment.handle);
        if let ResourceType::Image(rt_image) = resolved.resource {
            rts.push(rt_image);
        } else {
            panic!("A non-image resource was returned when attempting to resolve a render target");
        }
    }

    rts
}

fn create_image_memory_barrier() -> ImageBarrier {
    vk::PipelineStageFlags::
    ImageBarrier {
        handle: 0,
        source_stage: Default::default(),
        dest_stage: Default::default(),
        source_access: Default::default(),
        dest_access: Default::default(),
        old_layout: Default::default(),
        new_layout: Default::default()
    }
}

fn create_image_memory_barriers(
    resources: &[ResolvedResource],
    queue_index: u32,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout) -> Vec<vk::ImageMemoryBarrier> {

    let mut barriers: Vec<vk::ImageMemoryBarrier> = Vec::new();

    for resource in resources {
        if let ResourceType::Image(image) = &resource.resource {
            // TODO: range should not be static. Maybe pass in a slice of structs which include range values?
            let range = vk::ImageSubresourceRange::builder()
                .level_count(1)
                .base_mip_level(0)
                .layer_count(1)
                .base_array_layer(0)
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .build();
            // TODO: src and dst access masks should not be static
            let barrier = vk::ImageMemoryBarrier::builder()
                .image(image.image)
                .old_layout(old_layout)
                .new_layout(new_layout)
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .src_queue_family_index(queue_index)
                .dst_queue_family_index(queue_index)
                .subresource_range(range)
                .build();
            barriers.push(barrier);
        } else {
            panic!("Attempting to create an ImageMemoryBarrier for non-image resource");
        }
    }

    barriers
}

pub struct VulkanFrameGraph {
    pipeline_manager: VulkanPipelineManager,
    renderpass_manager: VulkanRenderpassManager
}

impl VulkanFrameGraph {
    pub fn new(
        renderpass_manager: VulkanRenderpassManager,
        pipeline_manager: VulkanPipelineManager) -> VulkanFrameGraph {

        VulkanFrameGraph {
            pipeline_manager,
            renderpass_manager
        }
    }

    fn compile(&mut self, nodes: &mut StableDiGraph<GraphicsPassNode, u32>, root_index: NodeIndex) -> Vec<NodeIndex>{
        // create input/output maps to detect graph edges
        let mut input_map = MultiMap::new();
        let mut output_map = MultiMap::new();
        for node_index in nodes.node_indices() {
            let node = &nodes[node_index];
            for input in node.get_dependencies() {
                input_map.insert(input, node_index);
            }
            for rt in node.get_writes() {
                output_map.insert(rt, node_index);
            }
        }

        // iterate over input map. For each input, find matching outputs and then
        // generate a graph edge for each pairing
        let mut unresolved_passes = Vec::new();
        for (input, node_index) in input_map.iter() {
            let find_outputs = output_map.get_vec(input);
            match find_outputs {
                Some(matched_outputs) => {
                    // input/output match defines a graph edge
                    for matched_output in matched_outputs {
                        // use update_edge instead of add_edge to avoid duplicates
                        self.nodes.update_edge(
                            *node_index,
                            *matched_output,
                            0);
                    }
                },
                _ => {
                    unresolved_passes.push(node_index);
                }
            }
        }

        // Use DFS to find all accessible nodes from the root node
        {
            let mut retained_nodes: Vec<bool> = Vec::new();
            retained_nodes.resize(nodes.node_count(), false);

            let mut dfs = Dfs::new(&nodes, root_index);
            while let Some(node_id) = dfs.next(&nodes) {
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
            let mut sort_result = petgraph::algo::toposort(nodes, None);
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

        self.compiled = true;
        sorted_nodes
    }

    fn link(
        &mut self,
        nodes: &mut StableDiGraph<GraphicsPassNode, u32>,
        sorted_nodes: &Vec<NodeIndex>) -> HashMap<ResourceHandle, vk::ImageLayout> {

        // All image bindings and attachments require the most recent usage for that resource
        // in case layout transitions are necessary. Since the graph has already been sorted,
        // we can just iterate over the sorted nodes to do this
        #[derive(Clone)]
        struct ResourceUsage {
            access: vk::AccessFlags,
            stage: vk::PipelineStageFlags,
            layout: Option<vk::ImageLayout>
        }
        let mut usage_cache: HashMap<ResourceHandle, ResourceUsage> = HashMap::new();
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

                    write_access | access > 0 || pipeline_write | stage > 0
                };

                for rt in node.get_rendertargets_mut() {
                    // rendertargets always write, so if this isn't the first usage of this resource
                    // then we know we need a barrier
                    let last_usage = usage_cache.get(&rt.handle);
                    let new_usage = ResourceUsage {
                        access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        layout: Some(new_usage.layout)
                    };
                    if let Some(usage) = last_usage {

                        let image_barrier = ImageBarrier {
                            handle: rt.handle,
                            source_stage: usage.stage,
                            dest_stage: new_usage.stage,
                            source_access: usage.access,
                            dest_access: new_usage.access,
                            old_layout: usage.layout.expect("Tried to get image layout from non-image"),
                            new_layout: new_usage.layout.expect("Should never hit this")
                        };
                        node.add_image_barrier(image_barrier);
                    }

                    usage_cache.insert(rt.handle, new_usage);
                }

                for input in node.get_inputs_mut() {
                    let last_usage = usage_cache.get(&input.handle);
                    // barrier required if:
                    //  * last usage was a write
                    //  * image layout has changed
                    let prev_write = {
                        if let Some(usage) = last_usage {
                            is_write(usage.access, usage.stage)
                        }
                        false
                    };
                    if let BindingType::Image(image_binding) = &mut input.binding_info.binding_type {
                        let new_usage = ResourceUsage{
                            access: input.binding_info.access,
                            stage: input.binding_info.stage,
                            layout: Some(image_binding.layout)
                        };

                        let layout_changed = {
                            if let Some(usage) = last_usage {
                                if let Some(layout) = usage.layout {
                                    layout != image_binding.layout
                                }
                            }
                            true
                        };

                        // need a barrier
                        if layout_changed || prev_write {
                            let old_layout = {
                                match old_usage.layout {
                                    Some(layout) => { layout },
                                    _ => { vk::ImageLayout::UNDEFINED } // TODO: not sure if this is actually valid
                                }
                            };
                            let image_barrier = ImageBarrier {
                                handle: input.handle,
                                source_stage: usage.stage,
                                dest_stage: new_usage.stage,
                                source_access: usage.access,
                                dest_access: new_usage.access,
                                old_layout: old_layout,
                                new_layout: new_usage.layout.unwrap()
                            };
                            node.add_image_barrier(image_barrier);
                        }

                        image_binding.layout = update_usage(input.handle, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    }
                }
            }
        }

        usage_cache
    }
}

impl FrameGraph for VulkanFrameGraph {
    type PN = GraphicsPassNode;
    type RPM = VulkanRenderpassManager;
    type PM = VulkanPipelineManager;
    type CB = vk::CommandBuffer;
    type RM = VulkanResourceManager;
    type RC = VulkanRenderContext;
    type Index = NodeIndex;

    fn start(&mut self, resource_manager: &VulkanResourceManager) -> Frame {
        Frame::new(resource_manager)
    }

    fn end(
        &mut self,
        mut frame: Frame,
        resource_manager: &Self::RM,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB) {

        frame.end();

        let nodes = frame.get_nodes();
        let resource_info = frame.get_create_info();
        let root_index = frame.get_root_index();

        // compile and link frame
        {
            let sorted_nodes = self.compile(nodes, root_index);
            let image_usage_cache = self.link(nodes, &sorted_nodes);
            frame.set_sorted_nodes(sorted_nodes);
            frame.set_image_usage_cache(image_usage_cache);
        }

        // excute nodes
        for index in frame.get_sorted_nodes() {
            let node = nodes.node_weight(*index).unwrap();

            let inputs = node.get_inputs();
            let outputs = node.get_outputs();
            let render_targets = node.get_rendertargets();
            let copy_sources = node.get_copy_sources();
            let copy_dests = node.get_copy_dests();

            // resolve copy sources and dests for this node
            let resolved_copy_sources = resolve_copy_resources(resource_manager, copy_sources);
            let resolved_copy_dests = resolve_copy_resources(resource_manager, copy_dests);

            // create image memory barriers for copies this node
            {
                let graphics_index = render_context.get_device().get_queue_family_indices().graphics
                    .expect("Expected a valid graphics queue index");

                // TODO: should not assume the old layout is color attachment
                let image_memory_barriers = create_image_memory_barriers(
                    &resolved_copy_sources.values().collect(),
                    graphics_index,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
            }

            // create memory barriers for copies for this node

            // write memory barriers to commandbuffer

            // prepare pipeline for execution (node's fill callback)
            let active_pipeline = node.get_pipeline_description();
            if let Some(pipeline_description) = active_pipeline {
                // resolve render targets for this node
                let resolved_render_targets = resolve_render_targets(resource_manager, render_targets);

                // Ensure all rendertargets are the same dimensions

                let renderpass = self.renderpass_manager.create_or_fetch_renderpass(
                    node,
                    node.get_rendertargets(),
                    render_context);

                let pipeline = self.pipeline_manager.create_pipeline(render_context, renderpass, pipeline_description);

                // create framebuffer
                // TODO: should cache framebuffer objects to avoid creating the same ones each frame

                // TODO: parameterize this per framebuffer attachment
                let clear_value = vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.1, 0.1, 0.1, 1.0]
                    }
                };

                // prepare and perform descriptor writes
                {

                }

                // begin render pass and bind pipeline

                // execute this node
            }
        }

    }

    fn end_old(
        &mut self,
        resource_manager: &mut Self::RM,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB) {

        match &self.sorted_nodes {
            Some(indices) => {
                for index in indices {
                    let node = nodes.node_weight(*index).unwrap();

                    let inputs = node.get_inputs();
                    let outputs = node.get_outputs();
                    let render_targets = node.get_rendertargets();
                    let copy_sources = node.get_copy_sources();
                    let copy_dests = node.get_copy_dests();

                    let mut resolved_inputs: ResolvedBindingMap = HashMap::new();
                    let mut resolved_outputs: ResolvedBindingMap = HashMap::new();

                    let (resolved_copy_sources, resolved_copy_dests) = {
                        let mut resolve_resource_type = | resources: &[ResourceHandle] | -> ResolvedResourceMap {
                            let mut resolved_map = ResolvedResourceMap::new();
                            for resource in resources {
                                let resolved = resource_manager.resolve_resource(resource);
                                resolved_map.insert(*resource, resolved.clone());
                            }
                            resolved_map
                        };

                        (resolve_resource_type(copy_sources), resolve_resource_type(copy_dests))
                    };

                    let resolved_render_targets = {
                        let mut rts: Vec<ImageWrapper> = Vec::new();
                        for rt_ref in render_targets {
                            let resolved = resource_manager.resolve_resource(&rt_ref.handle);
                            if let ResourceType::Image(rt_image) = resolved.resource {
                                rts.push(rt_image);
                            }
                        }
                        rts
                    };

                    let mut image_memory_barriers: Vec<vk::ImageMemoryBarrier> = Vec::new();
                    for (handle, resource) in &resolved_copy_sources {
                        if let ResourceType::Image(image) = &resource.resource {
                            let graphics_index = render_context.get_device().get_queue_family_indices().graphics
                                .expect("Expected a valid graphics queue index");
                            let range = vk::ImageSubresourceRange::builder()
                                .level_count(1)
                                .base_mip_level(0)
                                .layer_count(1)
                                .base_array_layer(0)
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .build();
                            let barrier = vk::ImageMemoryBarrier::builder()
                                .image(image.image)
                                // .old_layout(image.layout)
                                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                                .src_access_mask(vk::AccessFlags::NONE)
                                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                                .src_queue_family_index(graphics_index)
                                .dst_queue_family_index(graphics_index)
                                .subresource_range(range)
                                .build();
                            image_memory_barriers.push(barrier);
                        }
                    }
                    for (handle, resource) in &resolved_copy_dests {
                        if let ResourceType::Image(image) = &resource.resource {
                            let graphics_index = render_context.get_device().get_queue_family_indices().graphics
                                .expect("Expected a valid graphics queue index");
                            let range = vk::ImageSubresourceRange::builder()
                                .level_count(1)
                                .base_mip_level(0)
                                .layer_count(1)
                                .base_array_layer(0)
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .build();
                            let barrier = vk::ImageMemoryBarrier::builder()
                                .image(image.image)
                                .old_layout(image.layout)
                                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                                .src_access_mask(vk::AccessFlags::NONE)
                                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                                .src_queue_family_index(graphics_index)
                                .dst_queue_family_index(graphics_index)
                                .subresource_range(range)
                                .build();
                            image_memory_barriers.push(barrier);
                        }
                    }
                    unsafe {
                        render_context.get_device().get().cmd_pipeline_barrier(
                            *command_buffer,
                            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                            vk::PipelineStageFlags::FRAGMENT_SHADER,
                            vk::DependencyFlags::empty(),
                            &[],
                            &[],
                            &image_memory_barriers);
                    }

                    let active_pipeline = node.get_pipeline_description();
                    if let Some(pipeline_description) = active_pipeline {
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

                        let pipeline = self.pipeline_manager.create_pipeline(render_context, renderpass, pipeline_description);

                        let framebuffer = render_context.create_framebuffer(
                            renderpass,
                            &framebuffer_extent,
                            &resolved_render_targets);

                        let clear_value = vk::ClearValue {
                            color: vk::ClearColorValue {
                                float32: [0.1, 0.1, 0.1, 1.0]
                            }
                        };

                        // TODO: potential optimization in doing multiple descriptors in a single WriteDescriptorSet?
                        let mut descriptor_writes: Vec<vk::WriteDescriptorSet> = Vec::new();
                        // WriteDescriptorSet requires a reference to image or buffer bindings,
                        // so we have to keep these alive by placing them in vectors until after
                        // we call vkCommandUpdateDescriptorSets
                        let mut image_bindings: Vec<vk::DescriptorImageInfo> = Vec::new();
                        let mut buffer_bindings: Vec<vk::DescriptorBufferInfo> = Vec::new();
                        let (resolved_inputs, resolved_outputs) = {
                            let mut resolve_binding_type = | bindings: &[ResourceBinding] | -> ResolvedBindingMap {
                                let mut resolved_map = ResolvedBindingMap::new();
                                for binding in bindings {
                                    let descriptor_set = pipeline.descriptor_sets[binding.binding_info.set as usize];
                                    let mut descriptor_write_builder = vk::WriteDescriptorSet::builder()
                                        .dst_set(descriptor_set)
                                        .dst_binding(binding.binding_info.slot)
                                        .dst_array_element(0);
                                    let resolved = resource_manager.resolve_resource(&binding.handle);
                                    let mut image_info: vk::DescriptorImageInfo;
                                    let mut buffer_info: vk::DescriptorBufferInfo;
                                    match (&resolved.resource, &binding.binding_info.binding_type) {
                                        (ResourceType::Image(image), BindingType::Image(image_binding)) => {
                                            image_info = vk::DescriptorImageInfo::builder()
                                                .image_view(image.view)
                                                .image_layout(image.layout)
                                                .sampler(vk::Sampler::null()) // TODO: implement samplers
                                                .build();
                                            image_bindings.push(image_info);
                                            descriptor_write_builder = descriptor_write_builder
                                                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                                                .image_info(std::slice::from_ref(image_bindings.last().unwrap()));
                                        },
                                        (ResourceType::Buffer(buffer), BindingType::Buffer(buffer_binding)) => {
                                            buffer_info = vk::DescriptorBufferInfo::builder()
                                                .buffer(buffer.buffer)
                                                .offset(buffer_binding.offset) // TODO: support offsets for shared allocation buffers
                                                .range(buffer_binding.range)
                                                .build();
                                            buffer_bindings.push(buffer_info);
                                            descriptor_write_builder = descriptor_write_builder
                                                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                                                .buffer_info(std::slice::from_ref(buffer_bindings.last().unwrap()));
                                        },
                                        (_,_) => {
                                            panic!("Illegal combination of resource type and binding type provided");
                                        }
                                    };

                                    descriptor_writes.push(descriptor_write_builder.build());

                                    resolved_map.insert(
                                        binding.handle,
                                        ResolvedResourceBinding {
                                            resolved_resource: resolved});
                                }
                                resolved_map
                            };

                            (resolve_binding_type(inputs), resolve_binding_type(outputs))
                        };

                        // update and bind descriptors
                        unsafe {
                            // update descriptorsets
                            // TODO: support descriptor copies?
                            render_context.get_device().get().update_descriptor_sets(
                                &descriptor_writes,
                                &[]);
                            // bind descriptorsets
                            // TODO: COMPUTE SUPPORT
                            render_context.get_device().get().cmd_bind_descriptor_sets(
                                *command_buffer,
                                vk::PipelineBindPoint::GRAPHICS,
                                pipeline.pipeline_layout,
                                0,
                                &pipeline.descriptor_sets,
                                &[]);
                        }

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
                            render_context.get_device().get().cmd_begin_render_pass(
                                *command_buffer,
                                &render_pass_begin,
                                vk::SubpassContents::INLINE);

                            render_context.get_device().get().cmd_bind_pipeline(
                                *command_buffer,
                                vk::PipelineBindPoint::GRAPHICS,
                                pipeline.graphics_pipeline);
                        }
                    }

                    node.execute(
                        render_context,
                        command_buffer,
                        &resolved_inputs,
                        &resolved_outputs,
                        &resolved_copy_sources,
                        &resolved_copy_dests);

                    // if we began a render pass and bound a pipeline for this node, end it
                    if active_pipeline.is_some() {
                        unsafe {
                            render_context.get_device().get().cmd_end_render_pass(*command_buffer);
                        }
                    }
                }
            },
            _ => {
                println!("No nodes in framegraph to traverse");
            }
        }

        if let Some(sorted_indices) = &mut self.sorted_nodes {
            sorted_indices.clear();
        }
        self.nodes.clear();
        self.frame.reset();
        self.compiled = false;
        self.frame_started = false;
    }
}