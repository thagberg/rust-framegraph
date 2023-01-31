use std::ffi::c_void;
use ash::vk;
use context::api_types::buffer::BufferCreateInfo;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::pass_node::ResolvedBindingMap;
use framegraph::resource::vulkan_resource_manager::{ResolvedResourceMap, VulkanResourceManager};

use imgui::{DrawData, DrawVert, DrawIdx};

pub fn generate_pass(draw_data: &DrawData, resource_manager: &mut VulkanResourceManager) -> GraphicsPassNode {

    let vtx_create = vk::BufferCreateInfo::builder()
        .size((draw_data.total_vtx_count as usize * std::mem::size_of::<DrawVert>()) as vk::DeviceSize)
        .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .build();

    let idx_create = vk::BufferCreateInfo::builder()
        .size((draw_data.total_idx_count as usize * std::mem::size_of::<DrawIdx>()) as vk::DeviceSize)
        .usage(vk::BufferUsageFlags::INDEX_BUFFER)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .build();

    // let vtx_buffer = resource_manager.create_buffer_transient(
    //     BufferCreateInfo::new(vtx_create, "imgui_vtx_buffer".to_string()));

    // let idx_buffer = resource_manager.create_buffer_transient(
    //     BufferCreateInfo::new(idx_create, "imgui_idx_buffer".to_string()));

    // let uniform_buffer = resource_manager.create_buffer_persistent(
    //     BufferCreateInfo::new(ubo_create_info,
    //                           "ubo_persistent_buffer".to_string()));
    // let ubo_value = OffsetUBO {
    //     offset: [0.2, 0.1, 0.0]
    // };
    //
    // resource_manager.update_buffer(&uniform_buffer, |mapped_memory: *mut c_void| {
    //     unsafe {
    //         core::ptr::copy_nonoverlapping(
    //             &ubo_value,
    //             mapped_memory as *mut DrawVert,
    //             std::mem::size_of::<DrawVert>());
    //     };
    // });

    GraphicsPassNode::builder("imgui".to_string())
        .fill_commands(Box::new(
            move |render_ctx: &VulkanRenderContext,
                    command_buffer: &vk::CommandBuffer,
                    inputs: &ResolvedBindingMap,
                    outputs: &ResolvedBindingMap,
                    resolved_copy_sources: &ResolvedResourceMap,
                    resolved_copy_dests: &ResolvedResourceMap| {

            }
        ))
        .build()
        .expect("Failed to create imgui passnode")
}