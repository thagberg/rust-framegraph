use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use ash::vk;
use gpu_allocator::MemoryLocation;
use imgui::{DrawData, DrawVert, DrawIdx};

use context::api_types::buffer::BufferCreateInfo;
use context::api_types::device::{DeviceResource, DeviceWrapper};
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use framegraph::attachment::AttachmentReference;
use framegraph::binding::{BindingInfo, ResourceBinding};
use framegraph::graphics_pass_node::GraphicsPassNode;

pub fn generate_passes(
    draw_data: &DrawData,
    render_target: Rc<RefCell<DeviceResource>>,
    device: Rc<RefCell<DeviceWrapper>>) -> Vec<GraphicsPassNode> {

    let mut pass_nodes: Vec<GraphicsPassNode> = Vec::new();
    // one passnode per drawlist
    pass_nodes.reserve(draw_data.draw_lists_count());

    for draw_list in draw_data.draw_lists() {
        let vtx_create = BufferCreateInfo::new(vk::BufferCreateInfo::builder()
                                                   .size((draw_data.total_vtx_count as usize * std::mem::size_of::<DrawVert>()) as vk::DeviceSize)
                                                   .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                                                   .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                                   .build(),
                                               "imgui_vtx_buffer".to_string());

        let vtx_buffer = DeviceWrapper::create_buffer(
            device.clone(),
            &vtx_create,
            MemoryLocation::CpuToGpu);
        let vtx_data = draw_list.vtx_buffer();
        device.borrow().update_buffer(&vtx_buffer, |mapped_memory: *mut c_void, size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    vtx_data.as_ptr(),
                    mapped_memory as *mut DrawVert,
                    vtx_data.len()
                )
            }
        });

        let idx_create = BufferCreateInfo::new(vk::BufferCreateInfo::builder()
                                                   .size((draw_data.total_idx_count as usize * std::mem::size_of::<DrawIdx>()) as vk::DeviceSize)
                                                   .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                                                   .sharing_mode(vk::SharingMode::EXCLUSIVE)
                                                   .build(),
                                               "imgui_idx_buffer".to_string());

        let idx_buffer = DeviceWrapper::create_buffer(
            device.clone(),
            &idx_create,
            MemoryLocation::CpuToGpu);

        let idx_data = draw_list.idx_buffer();
        device.borrow().update_buffer(&idx_buffer, |mapped_memory: *mut c_void, size: u64| {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    idx_data.as_ptr(),
                    mapped_memory as *mut DrawIdx,
                    idx_data.len()
                )
            }
        });

        // let vtx_binding = ResourceBinding {
        //     resource: Rc::new(RefCell::new(())),
        //     binding_info: BindingInfo {
        //         binding_type: (),
        //         set: 0,
        //         slot: 0,
        //         stage: Default::default(),
        //         access: Default::default()
        //     }
        // };
        // let idx_binding = ResourceBinding {
        //     resource: Rc::new(RefCell::new(())),
        //     binding_info: BindingInfo {
        //         binding_type: (),
        //         set: 0,
        //         slot: 0,
        //         stage: Default::default(),
        //         access: Default::default()
        //     }
        // };

        let vtx_length = vtx_data.len() as u32;

        let rt_ref = AttachmentReference::new(
            render_target.clone(),
            vk::Format::R8G8B8A8_SRGB, // TODO: this should be parameterized
            vk::SampleCountFlags::TYPE_1,
            vk::AttachmentLoadOp::LOAD,
            vk::AttachmentStoreOp::STORE);

        let pass_node = GraphicsPassNode::builder("imgui".to_string())
            // .read(vtx_binding)
            // .read(idx_binding)
            .render_target(rt_ref)
            .fill_commands(Box::new(
                move |render_ctx: &VulkanRenderContext,
                      command_buffer: &vk::CommandBuffer | {
                    println!("Rendering Imgui drawlists");

                    unsafe {
                        render_ctx.get_device().borrow().get().cmd_draw(
                            *command_buffer,
                            vtx_length,
                            1,
                            0,
                            0);
                    }
                }
            ))
            .build()
            .expect("Failed to create imgui passnode");

        pass_nodes.push(pass_node);
    }

    pass_nodes
}