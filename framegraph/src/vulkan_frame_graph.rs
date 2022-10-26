extern crate petgraph;
use petgraph::{graph, stable_graph, Direction, Directed};
use petgraph::stable_graph::{Edges, NodeIndex};
extern crate multimap;
use multimap::MultiMap;

extern crate context;
use context::render_context::{RenderContext, CommandBuffer};

use ash::vk;
use crate::frame_graph::FrameGraph;
use crate::pass_node::PassNode;
use crate::binding::ResourceBinding;
use crate::graphics_pass_node::{GraphicsPassNode, ResolvedBinding, ResolvedBindingMap};
use crate::pipeline::{PipelineManager, VulkanPipelineManager};
use crate::resource::resource_manager::ResourceManager;
use crate::resource::vulkan_resource_manager::{ResolvedResource, ResolvedResourceMap, ResourceHandle, ResourceType, VulkanResourceManager};
use crate::renderpass_manager::{RenderpassManager, VulkanRenderpassManager};

use std::collections::HashMap;
use std::marker::PhantomData;
use petgraph::visit::EdgeRef;
use context::api_types::image::ImageWrapper;
use context::vulkan_render_context::VulkanRenderContext;

pub struct VulkanFrameGraph {
    nodes: stable_graph::StableDiGraph<GraphicsPassNode, u32>,
    frame_started: bool,
    compiled: bool,
    pipeline_manager: VulkanPipelineManager,
    renderpass_manager: VulkanRenderpassManager,
    sorted_nodes: Option<Vec<NodeIndex>>,
    root_index: Option<NodeIndex>
}

impl VulkanFrameGraph {
    pub fn new(
        renderpass_manager: VulkanRenderpassManager,
        pipeline_manager: VulkanPipelineManager) -> VulkanFrameGraph {

        VulkanFrameGraph {
            nodes: stable_graph::StableDiGraph::new(),
            frame_started: false,
            compiled: false,
            pipeline_manager,
            renderpass_manager,
            sorted_nodes: None,
            root_index: None
        }
    }

    fn _mark_unused(&self, visited_nodes: &mut Vec<bool>, edges: Edges<u32, Directed>)  {
        for edge in edges {
            let node_index = edge.source();
            visited_nodes[node_index.index()] = true;
            let incoming = self.nodes.edges_directed(node_index, Direction::Incoming);
            self._mark_unused(visited_nodes, incoming);
        }
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

    fn start(&mut self, root_node: Self::PN) {
        assert!(!self.frame_started, "Can't start a frame that's already been started");
        self.frame_started = true;
        self.root_index = Some(self.add_node(root_node));
    }

    fn add_node(&mut self, node: Self::PN) -> Self::Index {
        assert!(self.frame_started, "Can't add PassNode before frame has been started");
        assert!(!self.compiled, "Can't add PassNode after frame has been compiled");
        self.nodes.add_node(node)
    }

    fn compile(&mut self) {
        assert!(self.frame_started, "Can't compile FrameGraph before it's been started");
        assert!(!self.compiled, "FrameGraph has already been compiled");

        // create input/output maps to detect graph edges
        let mut input_map = MultiMap::new();
        let mut output_map = MultiMap::new();
        for node_index in self.nodes.node_indices() {
            let node = &self.nodes[node_index];
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
                        self.nodes.add_edge(
                            *matched_output,
                            *node_index,
                            0);
                    }
                },
                _ => {
                    unresolved_passes.push(node_index);
                }
            }
        }

        // need to also do a pass over the output map just to find unused
        // passes which can be culled from the framegraph
        let mut unused_passes = Vec::new();
        for (output, node_index) in output_map.iter() {
            let find_inputs = input_map.get_vec(output);
            match find_inputs {
                Some(_) => {
                    // These would have already been found during the input map pass
                },
                _ => {
                    unused_passes.push(node_index);
                }
            }
        }

        // Ensure root node is valid, then mark any passes which don't contribute to root node as unused
        let mut visited_nodes: Vec<bool> = vec![false; self.nodes.node_count()];
        match self.root_index {
            Some(root_index) => {
                let root_node = self.nodes.node_weight(root_index);
                match root_node {
                    Some(node) => {
                        visited_nodes[root_index.index()] = true;
                        let mut incoming = self.nodes.edges_directed(root_index, Direction::Incoming);
                        self._mark_unused(&mut visited_nodes, incoming);
                    },
                    None => {
                        panic!("Root node is invalid");
                    }
                }
            },
            None => {
                panic!("Root node was elided from frame graph, might have an unresolved dependency");
            }
        }

        // now remove unused passes
        for i in 0..visited_nodes.len() {
            if visited_nodes[i] == false {
                let node_index = NodeIndex::new(i);
                {
                    let node = self.nodes.node_weight(node_index).unwrap();
                    println!("Removing unused node: {:?}", node.get_name());
                }
                self.nodes.remove_node(node_index);
            }
        }

        if self.root_index.is_some() {
            let root_node = self.nodes.node_weight(self.root_index.unwrap());
        } else {
            panic!("Root node was elided from frame graph, might have an unresolved dependency");
        }

        // unresolved and unused passes have been removed from the graph,
        // so now we can use a topological sort to generate an execution order
        let sort_result = petgraph::algo::toposort(&self.nodes, None);
        match sort_result {
            Ok(sorted_list) => {
                for i in &sorted_list {
                    println!("Node: {:?}", self.nodes.node_weight(*i).unwrap().get_name());
                }
                self.sorted_nodes = Some(sorted_list);
            },
            Err(cycle_error) => {
                println!("A cycle was detected in the framegraph: {:?}", cycle_error);
            }
        }

        self.compiled = true;
    }

    fn end(
        &mut self,
        resource_manager: &mut Self::RM,
        render_context: &mut Self::RC,
        command_buffer: &Self::CB) {

        assert!(self.frame_started, "Can't end frame before it's been started");
        assert!(self.compiled, "Can't end frame before it's been compiled");
        match &self.sorted_nodes {
            Some(indices) => {
                for index in indices {
                    let node = self.nodes.node_weight(*index).unwrap();

                    let inputs = node.get_inputs();
                    let outputs = node.get_outputs();
                    let render_targets = node.get_rendertargets();
                    let copy_sources = node.get_copy_sources();
                    let copy_dests = node.get_copy_dests();

                    let mut resolve_resource_type = | resources: &[ResourceHandle] | -> ResolvedResourceMap {
                        let mut resolved_map = ResolvedResourceMap::new();
                        for resource in resources {
                            let resolved = resource_manager.resolve_resource(resource);
                            resolved_map.insert(*resource, resolved.clone());
                        }
                        resolved_map
                    };

                    let mut resolve_binding_type = | bindings: &[ResourceBinding] | -> ResolvedBindingMap {
                        let mut resolved_map = ResolvedBindingMap::new();
                        for binding in bindings {
                            let resolved = resource_manager.resolve_resource(&binding.handle);
                            resolved_map.inesrt(
                                binding.handle,
                                ResolvedBinding {
                                    binding: binding.clone(),
                                    resolved_resource: resolved});
                        }
                        resolved_map
                    };

                    let resolved_inputs = resolve_binding_type(inputs);
                    let resolved_outputs = resolve_binding_type(outputs);
                    // let resolved_render_targets = resolve_resource_type(render_targets);
                    let resolved_copy_sources = resolve_resource_type(copy_sources);
                    let resolved_copy_dests = resolve_resource_type(copy_dests);

                    let resolved_render_targets = {
                        let mut rts: Vec<ImageWrapper> = Vec::new();
                        for rt_handle in render_targets {
                            let resolved = resource_manager.resolve_resource(rt_handle);
                            if let ResourceType::Image(rt_image) = resolved.resource {
                                rts.push(rt_image);
                            }
                        }
                        rts
                    };

                    // Ensure all rendertargets are the same dimensions
                    let mut framebuffer_extent: Option<vk::Extent3D> = None;
                    {
                        for resolved in &resolved_render_targets {
                            match framebuffer_extent {
                                Some(extent) => {
                                    assert_eq!(extent, resolved.extent, "All framebuffer attachments must be the same dimensions");
                                },
                                None => {
                                    framebuffer_extent = Some(resolved.extent.clone());
                                }
                            }
                        }
                    }

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
                                .old_layout(image.layout)
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
                        let framebuffer_extent = framebuffer_extent
                            .expect("Framebuffer required for renderpass");

                        let renderpass = self.renderpass_manager.create_or_fetch_renderpass(
                            node,
                            resource_manager,
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
        self.compiled = false;
        self.frame_started = false;
    }
}