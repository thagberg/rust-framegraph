mod utility;
use crate::{
    utility::constants::*,
    utility::debug::*,
    utility::share,
};

use ash::vk;
use winit::event::{Event, VirtualKeyCode, ElementState, KeyboardInput, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use glam::IVec2;

use std::ptr;

extern crate framegraph;
extern crate context;
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use context::api_types::surface::SurfaceWrapper;
use context::api_types::swapchain::SwapchainWrapper;
use context::api_types::device::DeviceWrapper;
use context::api_types::instance::InstanceWrapper;
use context::api_types::vulkan_command_buffer::VulkanCommandBuffer;
use framegraph::resource::vulkan_resource_manager::{ResourceHandle, VulkanResourceManager};
use framegraph::shader::ShaderManager;
use framegraph::graphics_pass_node::GraphicsPassNode;
use framegraph::frame_graph::FrameGraph;
use framegraph::vulkan_frame_graph::VulkanFrameGraph;
use framegraph::renderpass_manager::VulkanRenderpassManager;
use framegraph::pipeline::VulkanPipelineManager;
use framegraph::resource::resource_manager::ResourceManager;
use passes::blit;

mod examples;
use crate::examples::uniform_buffer::ubo_pass::UBOPass;
// use crate::examples::uniform_buffer::transient_input_pass::TransientInputPass;


// Constants
const WINDOW_TITLE: &'static str = "Framegraph Renderer";
const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct SyncObjects {
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    inflight_fences: Vec<vk::Fence>,
}

struct VulkanApp {
    window: winit::window::Window,
    // debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_merssager: vk::DebugUtilsMessengerEXT,

    render_context: VulkanRenderContext,
    resource_manager: VulkanResourceManager,

    ubo_pass: UBOPass,
    frame_graph: VulkanFrameGraph,
    // ubo_pass: UBOPass,
    // transient_pass: TransientInputPass,

    swapchain_handles: Vec<ResourceHandle>,
    swapchain_framebuffers: Vec<vk::Framebuffer>,

    render_pass: vk::RenderPass,
    // pipeline_layout: vk::PipelineLayout,
    // graphics_pipeline: vk::Pipeline,

    command_buffers: Vec<vk::CommandBuffer>,

    shader_manager: ShaderManager,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
}

impl VulkanApp {
    pub fn new(event_loop: &winit::event_loop::EventLoop<()>) -> VulkanApp {
        let window = utility::window::init_window(event_loop, WINDOW_TITLE, WINDOW_WIDTH, WINDOW_HEIGHT);

        // init vulkan stuff
        let entry = ash::Entry::linked();
        let instance = share::create_instance(
            &entry,
            WINDOW_TITLE,
            VALIDATION.is_enable,
            &VALIDATION.required_validation_layers.to_vec(),
        );
        let (debug_utils_loader, debug_merssager) =
            setup_debug_utils(VALIDATION.is_enable, &entry, &instance);

        let surface_wrapper = SurfaceWrapper::new(
            &entry,
            &instance,
            &window
        );

        let render_context = VulkanRenderContext::new(
            entry,
            instance,
            debug_utils_loader,
            Some(surface_wrapper),
            &window);

        assert!(render_context.get_swapchain().is_some(), "Can't continue without valid swapchain");
        let (swapchain_extent, swapchain_format) = {
            let swapchain = &render_context.get_swapchain().as_ref().unwrap();
            (swapchain.get_extent(), swapchain.get_format())
        };

        let render_pass = VulkanApp::create_render_pass(
            render_context.get_device().get(),
            swapchain_format);
        // let (graphics_pipeline, pipeline_layout) = share::v1::create_graphics_pipeline(
        //     render_context.get_device(),
        //     render_pass,
        //     swapchain_extent);
        let swapchain_framebuffers = {
            assert!(render_context.get_swapchain().is_some(), "Can't continue without swapchain");
            let swapchain = render_context.get_swapchain().as_ref().unwrap();
            let image_views: Vec<vk::ImageView> = swapchain.get_images().iter()
                .map(|s| s.view).collect();
            share::v1::create_framebuffers(
                render_context.get_device().get(),
                render_pass,
                &image_views,
                swapchain_extent)
        };

        // let ubo_pass = UBOPass::new(&mut render_context);
        // let transient_pass = TransientInputPass::new(
        //     &mut render_context,
        //     ubo_pass.render_target);

        let mut resource_manager = VulkanResourceManager::new(
            render_context.get_instance(),
            render_context.get_device_wrapper(),
            render_context.get_physical_device());

        let pipeline_manager = VulkanPipelineManager::new();

        let ubo_pass = UBOPass::new(&mut resource_manager);

        let frame_graph = VulkanFrameGraph::new(VulkanRenderpassManager::new(), pipeline_manager);

        let shader_manager = ShaderManager::new();

        let command_buffers = share::v1::create_command_buffers(
            render_context.get_device().get(),
            render_context.get_graphics_command_pool(),
            2);
        let sync_ojbects = VulkanApp::create_sync_objects(
            render_context.get_device().get());

        // cleanup(); the 'drop' function will take care of it.

        let mut swapchain_handles: Vec<ResourceHandle> = Vec::new();
        if let Some(swapchain) = render_context.get_swapchain().as_ref() {
            for (index, image) in swapchain.get_images().into_iter().enumerate() {
               let handle = resource_manager.register_image(image, &format!("Swapchain{}", index));
                swapchain_handles.push(handle);
            }
        }

        // let mut swapchain_handles = vec![];
        // if let Some(swaps) = &swapchain {
        //     for image in swaps.get_images()
        //     {
        //         let handle = resource_manager.register_image(image);
        //         swapchain_handles.push(handle);
        //     }
        // }

        VulkanApp {
            window,
            // debug_utils_loader,
            debug_merssager,

            render_context,
            resource_manager,
            ubo_pass,
            frame_graph,
            // ubo_pass,
            // transient_pass,

            swapchain_handles,
            swapchain_framebuffers,

            // pipeline_layout,
            render_pass,
            // graphics_pipeline,

            command_buffers,

            shader_manager,

            image_available_semaphores: sync_ojbects.image_available_semaphores,
            render_finished_semaphores: sync_ojbects.render_finished_semaphores,
            in_flight_fences: sync_ojbects.inflight_fences,
            current_frame: 0,
        }
    }

    fn draw_frame(&mut self) {
        let wait_fences = [self.in_flight_fences[self.current_frame]];

        unsafe
        {
            self.render_context.get_device().get()
                .device_wait_idle()
                .expect("Error while waiting for device to idle");
            self.render_context.get_device().get()
                .wait_for_fences(&wait_fences, true, u64::MAX)
                .expect("Failed to wait for Fence!");
        }

        let (_, image_index) = self.render_context.get_swapchain().as_ref().unwrap().acquire_next_image(
            None,
        Some(self.image_available_semaphores[self.current_frame]),
        None);

        self.resource_manager.flush(self.render_context.get_device());

        let command_buffer = self.command_buffers[image_index as usize];
        unsafe {
            self.render_context.get_device().get().reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::empty())
                .expect("Failed to reset command buffer");

            let command_buffer_begin_info = vk::CommandBufferBeginInfo {
                s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
                p_next: ptr::null(),
                p_inheritance_info: ptr::null(),
                flags: vk::CommandBufferUsageFlags::SIMULTANEOUS_USE,
            };
            self.render_context.get_device().get().begin_command_buffer(command_buffer, &command_buffer_begin_info)
                .expect("Failed to begin recording command buffer");
        }

        // let surface_extent = self.render_context.get_swapchain().as_ref().unwrap().get_extent();
        {
            let swapchain_handle = self.swapchain_handles[image_index as usize];
            let extent = self.render_context.get_swapchain().as_ref().unwrap().get_extent();
            let blit_offsets = [glam::IVec2::new(0, 0), glam::IVec2::new(extent.width as i32, extent.height as i32)];
            let (ubo_pass_node, ubo_render_target) = self.ubo_pass.generate_pass(&mut self.resource_manager, self.render_context.get_swapchain().as_ref().unwrap().get_extent());
            self.frame_graph.start(blit::generate_pass(ubo_render_target, 0, swapchain_handle, 0, blit_offsets));
            self.frame_graph.add_node(ubo_pass_node);
            self.frame_graph.compile();
            self.frame_graph.end(
                &mut self.resource_manager,
                &mut self.render_context,
                &command_buffer);

            let present_transition = {
                let swapchain_image = self.render_context.get_swapchain().as_ref().unwrap().get_images()[image_index as usize].image;
                let subresource_range = vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .base_array_layer(0)
                    .level_count(1)
                    .base_mip_level(0)
                    .build();
                vk::ImageMemoryBarrier::builder()
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::NONE)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(swapchain_image)
                    .subresource_range(subresource_range)
                    .build()
            };

            unsafe {
                self.render_context.get_device().get().cmd_pipeline_barrier(
                    command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[present_transition]);
            }
        }

        unsafe {
            let device = self.render_context.get_device();
            // device.cmd_end_render_pass(command_buffer);
            device.get().end_command_buffer(command_buffer)
                .expect("Failed to record command buffer");
        }


        let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished_semaphores[self.current_frame]];

        let command_buffers = [command_buffer];
        let submit_infos = [vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            p_next: ptr::null(),
            wait_semaphore_count: wait_semaphores.len() as u32,
            p_wait_semaphores: wait_semaphores.as_ptr(),
            p_wait_dst_stage_mask: wait_stages.as_ptr(),
            command_buffer_count: 1,
            p_command_buffers: command_buffers.as_ptr(),
            signal_semaphore_count: signal_semaphores.len() as u32,
            p_signal_semaphores: signal_semaphores.as_ptr(),
        }];

        unsafe {
            // self.device
            self.render_context.get_device().get()
                .reset_fences(&wait_fences)
                .expect("Failed to reset Fence!");

            // self.device
            self.render_context.get_device().get()
                .queue_submit(
                    // self.graphics_queue,
                    self.render_context.get_graphics_queue(),
                    &submit_infos,
                    self.in_flight_fences[self.current_frame],
                )
                .expect("Failed to execute queue submit.");
        }

        let swapchains = [self.render_context.get_swapchain().as_ref().unwrap().get()];

        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(std::slice::from_ref(&image_index));

        unsafe {
            self.render_context.get_swapchain().as_ref().unwrap().get_loader()
                .queue_present(
                    self.render_context.get_present_queue(),
                    &present_info)
                .expect("Failed to execute queue present.");
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }

    fn create_render_pass(device: &ash::Device, surface_format: vk::Format) -> vk::RenderPass {
        let color_attachment = vk::AttachmentDescription {
            format: surface_format,
            flags: vk::AttachmentDescriptionFlags::empty(),
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        };

        let color_attachment_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };

        let subpasses = [vk::SubpassDescription {
            color_attachment_count: 1,
            p_color_attachments: &color_attachment_ref,
            p_depth_stencil_attachment: ptr::null(),
            flags: vk::SubpassDescriptionFlags::empty(),
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            input_attachment_count: 0,
            p_input_attachments: ptr::null(),
            p_resolve_attachments: ptr::null(),
            preserve_attachment_count: 0,
            p_preserve_attachments: ptr::null(),
        }];

        let render_pass_attachments = [color_attachment];

        let subpass_dependencies = [vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dependency_flags: vk::DependencyFlags::empty(),
        }];

        let renderpass_create_info = vk::RenderPassCreateInfo {
            s_type: vk::StructureType::RENDER_PASS_CREATE_INFO,
            flags: vk::RenderPassCreateFlags::empty(),
            p_next: ptr::null(),
            attachment_count: render_pass_attachments.len() as u32,
            p_attachments: render_pass_attachments.as_ptr(),
            subpass_count: subpasses.len() as u32,
            p_subpasses: subpasses.as_ptr(),
            dependency_count: subpass_dependencies.len() as u32,
            p_dependencies: subpass_dependencies.as_ptr(),
        };

        unsafe {
            device
                .create_render_pass(&renderpass_create_info, None)
                .expect("Failed to create render pass!")
        }
    }

    fn create_sync_objects(device: &ash::Device) -> SyncObjects {
        let mut sync_objects = SyncObjects {
            image_available_semaphores: vec![],
            render_finished_semaphores: vec![],
            inflight_fences: vec![],
        };

        let semaphore_create_info = vk::SemaphoreCreateInfo {
            s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::SemaphoreCreateFlags::empty(),
        };

        let fence_create_info = vk::FenceCreateInfo {
            s_type: vk::StructureType::FENCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::FenceCreateFlags::SIGNALED,
        };

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            unsafe {
                let image_available_semaphore = device
                    .create_semaphore(&semaphore_create_info, None)
                    .expect("Failed to create Semaphore Object!");
                let render_finished_semaphore = device
                    .create_semaphore(&semaphore_create_info, None)
                    .expect("Failed to create Semaphore Object!");
                let inflight_fence = device
                    .create_fence(&fence_create_info, None)
                    .expect("Failed to create Fence Object!");

                sync_objects
                    .image_available_semaphores
                    .push(image_available_semaphore);
                sync_objects
                    .render_finished_semaphores
                    .push(render_finished_semaphore);
                sync_objects.inflight_fences.push(inflight_fence);
            }
        }

        sync_objects
    }
}

impl Drop for VulkanApp {
    fn drop(&mut self) {
        unsafe {
            let device = self.render_context.get_device().get();
            for i in 0..MAX_FRAMES_IN_FLIGHT {
                device.destroy_semaphore(self.image_available_semaphores[i], None);
                device.destroy_semaphore(self.render_finished_semaphores[i], None);
                device.destroy_fence(self.in_flight_fences[i], None);
            }

            // device.destroy_command_pool(self.command_pool, None);

            for &framebuffer in self.swapchain_framebuffers.iter() {
                device.destroy_framebuffer(framebuffer, None);
            }

            // device.destroy_pipeline(self.graphics_pipeline, None);
           // device .destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_render_pass(self.render_pass, None);

            // for &imageview in self.swapchain_imageviews.iter() {
            //     device.destroy_image_view(imageview, None);
            // }

            // self.swapchain_loader
            //     .destroy_swapchain(self.swapchain, None);
            // device.destroy_device(None);
            // self.surface_loader.destroy_surface(self.surface, None);

            // if VALIDATION.is_enable {
            //     self.debug_utils_loader
            //         .destroy_debug_utils_messenger(self.debug_merssager, None);
            // }

            // self.render_context.get_instance().destroy_instance(None);
            // self.instance.destroy_instance(None);
        }
    }
}

// Fix content -------------------------------------------------------------------------------
impl VulkanApp {

    pub fn main_loop(mut self, event_loop: EventLoop<()>) {

        event_loop.run(move |event, _, control_flow| {

            match event {
                | Event::WindowEvent { event, .. } => {
                    match event {
                        | WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit
                        },
                        | WindowEvent::KeyboardInput { input, .. } => {
                            match input {
                                | KeyboardInput { virtual_keycode, state, .. } => {
                                    match (virtual_keycode, state) {
                                        | (Some(VirtualKeyCode::Escape), ElementState::Pressed) => {
                                            *control_flow = ControlFlow::Exit
                                        },
                                        | _ => {},
                                    }
                                },
                            }
                        },
                        | _ => {},
                    }
                },
                | Event::MainEventsCleared => {
                    self.window.request_redraw();
                },
                | Event::RedrawRequested(_window_id) => {
                    self.draw_frame();
                },
                | Event::LoopDestroyed => {
                    unsafe {
                        self.render_context.get_device().get().device_wait_idle()
                            .expect("Failed to wait device idle!")
                    };
                },
                _ => (),
            }

        })
    }
}

fn main() {

    let event_loop = EventLoop::new();

    let vulkan_app = VulkanApp::new(&event_loop);
    vulkan_app.main_loop(event_loop);
}
// -------------------------------------------------------------------------------------------
