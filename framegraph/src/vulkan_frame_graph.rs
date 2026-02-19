use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::DerefMut;
use std::sync::{Mutex, RwLock};
use ash::vk;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use tracing::{span, Level, warn, trace};
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;

use context::render_context::RenderContext;
use crate::frame::Frame;
use crate::frame_graph::FrameGraph;
use crate::graphics_pass_node::{GraphicsPassNode};
use crate::pipeline::VulkanPipelineManager;
use crate::renderpass_manager::VulkanRenderpassManager;

use crate::compiler::compile;
use crate::executor::{execute_graphics_node, execute_compute_node, execute_copy_node};
use crate::linker::{ResourceUsage, NodeBarriers, translate_barriers, link_graphics_node, link_copy_node, link_compute_node, link_present_node};
use api_types::device::interface::DeviceInterface;
use context::vulkan_render_context::VulkanRenderContext;
use profiling::enter_span;
use crate::command_list::{CommandList};
use crate::pass_type::PassType;

#[derive(Debug)]
pub struct VulkanFrameGraph {
    pipeline_manager: Mutex<VulkanPipelineManager>,
    renderpass_manager: VulkanRenderpassManager,
    node_barriers: HashMap<NodeIndex, NodeBarriers>
}

impl Drop for VulkanFrameGraph {
    fn drop(&mut self) {
        println!("Dropping VulkanFrameGraph");
    }
}

impl VulkanFrameGraph {
    pub fn new() -> VulkanFrameGraph {

        VulkanFrameGraph {
            pipeline_manager: Mutex::new(VulkanPipelineManager::new()),
            renderpass_manager: VulkanRenderpassManager::new(),
            node_barriers: HashMap::new()
        }
    }

    /// Links the nodes in the framegraph based on their dependencies
    ///
    /// The goal of linking is to ensure that where there are matching
    /// outputs of one pass into the inputs of another pass, we apply
    /// appropriate memory barriers and also ensure that resource
    /// transitions are correct.
    ///
    /// Currently this must be done synchronously (though not necessarily
    /// on the main thread), although it's possible in the future we could
    /// identify multiple non-overlapping paths through the topoligically-
    /// sorted graph to link separately.
    #[tracing::instrument]
    fn link(
        &mut self,
        nodes: &mut StableDiGraph<RwLock<PassType>, u32>,
        sorted_nodes: &[NodeIndex]) -> Vec<CommandList> {

        let mut command_lists: Vec<CommandList> = Vec::new();
        let mut current_list = CommandList::new();

        // All image bindings and attachments require the most recent usage for that resource
        // in case layout transitions are necessary. Since the graph has already been sorted,
        // we can just iterate over the sorted nodes to do this
        let mut usage_cache: HashMap<u64, ResourceUsage> = HashMap::new();
        for node_index in sorted_nodes {
            // let mut node_borrow = nodes.node_weight_mut(*node_index);
            let node_borrow = nodes.node_weight(*node_index);
            // let node_lock = nodes.node_weight(*node_index);
            // if let Some(node) = nodes.node_weight_mut(*node_index) {
            if let Some(node_lock) = node_borrow {
                let mut node = node_lock.write().unwrap();
                let node_barrier = match node.deref_mut() {
                    PassType::Graphics(gn) => {
                        link_graphics_node(gn, &mut usage_cache)
                    }
                    PassType::Copy(cn) => {
                        link_copy_node(cn, &mut usage_cache)
                    },
                    PassType::Compute(cn) => {
                        link_compute_node(cn, &mut usage_cache)
                    }
                    PassType::Present(pn) => {
                        link_present_node(pn, &mut usage_cache)
                    }
                };

                current_list.nodes.push(*node_index);

                self.node_barriers.insert(*node_index, node_barrier);
            }
        }

        command_lists.push(current_list);
        command_lists
    }
}

impl FrameGraph for VulkanFrameGraph {
    type PN = GraphicsPassNode;
    type RPM = VulkanRenderpassManager;
    type PM = VulkanPipelineManager;
    type CB = vk::CommandBuffer;
    type RC = VulkanRenderContext;
    type Index = NodeIndex;

    fn start(
        &mut self,
        device: DeviceInterface,
        descriptor_pool: vk::DescriptorPool) -> Box<Frame> {
        // let span = span!(Level::TRACE, "Framegraph Start");
        // let _enter = span.enter();

        Box::new(Frame::new(device, descriptor_pool))
    }

    fn end(
        &mut self,
        frame: &mut Frame,
        // render_context: &'d mut Self::RC,
        render_context: &Self::RC,
        command_buffer: &Self::CB) {

        let span = span!(Level::TRACE, "Framegraph End");
        let _enter = span.enter();

        frame.end();

        let root_index = frame.get_root_index();

        // compile and link frame
        let command_lists = {
            let sorted_nodes = compile(&mut frame.nodes, root_index);
            // let dot = petgraph::dot::Dot::with_config(&frame.nodes, &[petgraph::dot::Config::EdgeIndexLabel]);
            self.link(&mut frame.nodes, &sorted_nodes)
        };

        // add a global memory barrier to ensure all CPU writes are accessible
        // prior to dispatching GPU work
        {
            let host_barrier = vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::HOST_WRITE)
                .dst_access_mask(vk::AccessFlags::UNIFORM_READ
                    | vk::AccessFlags::INDEX_READ
                    | vk::AccessFlags::VERTEX_ATTRIBUTE_READ);

            unsafe {
                render_context.get_device().get().cmd_pipeline_barrier(
                    *command_buffer,
                    vk::PipelineStageFlags::HOST,
                    vk::PipelineStageFlags::VERTEX_INPUT | vk::PipelineStageFlags::VERTEX_SHADER,
                    vk::DependencyFlags::empty(),
                    &[host_barrier],
                    &[],
                    &[]);
            }
        }

        // excute nodes
        // let sorted_nodes = &frame.sorted_nodes;
        command_lists.par_iter().for_each(|command_list| {

            enter_span!(tracing::Level::TRACE, "Filling command lists");
            // let nodes = &mut frame.nodes;
            let descriptor_sets = frame.descriptor_sets.clone();
            let descriptor_pool = frame.descriptor_pool.clone();
            let nodes = &frame.nodes;
            for index in &command_list.nodes {
                enter_span!(tracing::Level::TRACE, "Node", "{}", index.index());
                // Gets mutable ref of all nodes for each parallel commandlist?
                // let node = nodes.node_weight_mut(*index).unwrap();
                let mut node = nodes.node_weight(*index).unwrap().write().unwrap();
                render_context.get_device().push_debug_label(*command_buffer, node.get_name());

                // Prepare and execute resource barriers
                let barriers = self.node_barriers.get(index);
                if let Some(barriers) = barriers {
                    enter_span!(tracing::Level::TRACE, "Generate barriers");

                    let translation = translate_barriers(barriers, render_context);

                    if translation.image_barriers.len() > 0 || translation.buffer_barriers.len() > 0 {
                        unsafe {
                            render_context.get_device()
                                .get().cmd_pipeline_barrier(
                                *command_buffer,
                                translation.source_stage,
                                translation.dest_stage,
                                vk::DependencyFlags::empty(),
                                &[],
                                &translation.buffer_barriers,
                                &translation.image_barriers);
                        }
                    }
                }

                // prepare pipeline for execution (node's fill callback)
                {
                    let node_name = node.get_name();
                    trace!(target: "framegraph", "Executing node: {node_name}");
                }
                match node.deref_mut() {
                    PassType::Graphics(graphics_node) => {
                        execute_graphics_node(&self.renderpass_manager, &self.pipeline_manager, descriptor_sets.clone(), descriptor_pool, render_context, command_buffer, graphics_node);
                    },
                    PassType::Copy(copy_node) => {
                        execute_copy_node(descriptor_sets.clone(), descriptor_pool, render_context, command_buffer, copy_node);
                    },
                    PassType::Compute(compute_node) => {
                        execute_compute_node(
                            &self.pipeline_manager,
                            descriptor_sets.clone(),
                            descriptor_pool,
                            render_context.get_device(),
                            *command_buffer,
                            compute_node);
                    }
                    _ => {}
                }

                render_context.get_device()
                    .pop_debug_label(*command_buffer);
            }
        });

    }
}