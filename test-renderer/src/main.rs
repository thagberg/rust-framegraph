mod utility;

use std::ptr;
use std::rc::Rc;
use std::cell::RefCell;

use crate::{
    utility::constants::*,
    utility::debug::*,
    utility::share,
};

use ash::vk;
use winit::event::{Event, VirtualKeyCode, ElementState, KeyboardInput, WindowEvent, MouseButton};
use winit::event_loop::{EventLoop, ControlFlow};
use imgui::Context;

extern crate framegraph;
extern crate context;
use context::render_context::RenderContext;
use context::vulkan_render_context::VulkanRenderContext;
use context::api_types::surface::SurfaceWrapper;
use context::api_types::device::{DeviceResource, ResourceType};
use framegraph::frame::Frame;
use framegraph::frame_graph::FrameGraph;
use framegraph::vulkan_frame_graph::VulkanFrameGraph;
use framegraph::renderpass_manager::VulkanRenderpassManager;
use framegraph::pipeline::VulkanPipelineManager;
use passes::{blit, imgui_draw};
use passes::imgui_draw::ImguiRender;

mod examples;
use crate::examples::uniform_buffer::ubo_pass::UBOPass;


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


    ubo_pass: UBOPass,
    frames: [Option<Box<Frame>>; MAX_FRAMES_IN_FLIGHT],
    // ubo_pass: UBOPass,
    // transient_pass: TransientInputPass,

    swapchain_images: Vec<Rc<RefCell<DeviceResource>>>,
    swapchain_framebuffers: Vec<vk::Framebuffer>,

    render_pass: vk::RenderPass,
    // pipeline_layout: vk::PipelineLayout,
    // graphics_pipeline: vk::Pipeline,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,

    imgui: Context,
    imgui_renderer: ImguiRender,

    frame_graph: VulkanFrameGraph,
    render_context: VulkanRenderContext
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
            render_context.get_device().borrow().get(),
            swapchain_format);
        // let (graphics_pipeline, pipeline_layout) = share::v1::create_graphics_pipeline(
        //     render_context.get_device(),
        //     render_pass,
        //     swapchain_extent);
        let swapchain_framebuffers = {
            assert!(render_context.get_swapchain().is_some(), "Can't continue without swapchain");
            let swapchain = render_context.get_swapchain().as_ref().unwrap();
            let image_views: Vec<vk::ImageView> = swapchain.get_images().iter()
                .map(|s| {
                    let image = s.borrow();
                    if let Some(resource) = &image.resource_type {
                        match &resource {
                            ResourceType::Image(swapchain_image) => {
                                swapchain_image.view
                            },
                            _ => {
                                panic!("Non-image resource type in swapchain")
                            }
                        }
                    } else {
                        panic!("All swapchain resources should be valid")
                    }
                }).collect();
            share::v1::create_framebuffers(
                render_context.get_device().borrow().get(),
                render_pass,
                &image_views,
                swapchain_extent)
        };

        let pipeline_manager = VulkanPipelineManager::new();

        let ubo_pass = UBOPass::new(render_context.get_device());

        let frame_graph = VulkanFrameGraph::new(VulkanRenderpassManager::new(), pipeline_manager);

        let sync_ojbects = VulkanApp::create_sync_objects(
            render_context.get_device().borrow().get());

        // cleanup(); the 'drop' function will take care of it.

        let mut swapchain_images: Vec<Rc<RefCell<DeviceResource>>> = Vec::new();
        if let Some(swapchain) = render_context.get_swapchain().as_ref() {
            for (_index, image) in swapchain.get_images().into_iter().enumerate() {
                swapchain_images.push(image.clone());
            }
        }

        let mut imgui = Context::create();
        imgui.set_ini_filename(None);
        let font_texture = {
            let mut imgui_io = imgui.io_mut();
            imgui_io.display_size = [swapchain_extent.width as f32, swapchain_extent.height as f32];
            let fonts = imgui.fonts();
            fonts.build_rgba32_texture()
        };

        let imgui_renderer = imgui_draw::ImguiRender::new(
            render_context.get_device().clone(),
            &render_context,
            font_texture);

        VulkanApp {
            window,
            // debug_utils_loader,
            debug_merssager,

            render_context,
            ubo_pass,
            frame_graph,
            frames: Default::default(),

            swapchain_images,
            swapchain_framebuffers,

            render_pass,

            image_available_semaphores: sync_ojbects.image_available_semaphores,
            render_finished_semaphores: sync_ojbects.render_finished_semaphores,
            in_flight_fences: sync_ojbects.inflight_fences,
            current_frame: 0,

            imgui,
            imgui_renderer
        }
    }

    fn draw_frame(&mut self, mouse_pos: (f32, f32), mouse_down: bool) {
        let wait_fences = [self.in_flight_fences[self.current_frame]];

        unsafe
        {
            self.render_context.get_device().borrow().get()
                .device_wait_idle()
                .expect("Error while waiting for device to idle");
            self.render_context.get_device().borrow().get()
                .wait_for_fences(&wait_fences, true, u64::MAX)
                .expect("Failed to wait for Fence!");
        }

        let (_, image_index) = self.render_context.get_swapchain().as_ref().unwrap().acquire_next_image(
            None,
        Some(self.image_available_semaphores[self.current_frame]),
        None);

        let command_buffer = self.render_context.get_graphics_command_buffer(image_index as usize);
        unsafe {
            self.render_context.get_device().borrow().get().reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::empty())
                .expect("Failed to reset command buffer");

            let command_buffer_begin_info = vk::CommandBufferBeginInfo {
                s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
                p_next: ptr::null(),
                p_inheritance_info: ptr::null(),
                flags: vk::CommandBufferUsageFlags::SIMULTANEOUS_USE,
            };
            self.render_context.get_device().borrow().get().begin_command_buffer(command_buffer, &command_buffer_begin_info)
                .expect("Failed to begin recording command buffer");
        }

        // let surface_extent = self.render_context.get_swapchain().as_ref().unwrap().get_extent();
        {
            {
                let mut imgui_io = self.imgui.io_mut();
                imgui_io.mouse_pos = [mouse_pos.0, mouse_pos.1];
                imgui_io.mouse_down[0] = mouse_down;
            }
            let ui = self.imgui.new_frame();
            ui.text("Testing UI");
            let ui_draw_data = self.imgui.render();


            let swapchain_resource = self.swapchain_images[image_index as usize].clone();
            let extent = self.render_context.get_swapchain().as_ref().unwrap().get_extent();
            let blit_offsets = [glam::IVec2::new(0, 0), glam::IVec2::new(extent.width as i32, extent.height as i32)];
            self.frames[self.current_frame] = Some(self.frame_graph.start());
            let current_frame = self.frames[self.current_frame].as_mut().unwrap();
            //self.frame_graph.start(blit::generate_pass(ubo_render_target, 0, swapchain_handle, 0, blit_offsets));
            let (ubo_pass_node, ubo_render_target) = self.ubo_pass.generate_pass(self.render_context.get_device(), self.render_context.get_swapchain().as_ref().unwrap().get_extent());
            let blit_node = blit::generate_pass(ubo_render_target.clone(), 0, swapchain_resource.clone(), 0, blit_offsets);
            let imgui_nodes = self.imgui_renderer.generate_passes(ui_draw_data, ubo_render_target.clone(), self.render_context.get_device());
            current_frame.start(blit_node);
            current_frame.add_node(ubo_pass_node);
            for imgui_node in imgui_nodes {
                current_frame.add_node(imgui_node);
            }
            self.frame_graph.end(
                current_frame,
                &mut self.render_context,
                &command_buffer);

            // TODO: this should be handled analytically, rather than just expecting that the swap
            //      image was used as a transfer dest
            let present_transition = {
                let swapchain_resource = self.swapchain_images[image_index as usize].clone();
                let swapchain_image = swapchain_resource.borrow();
                if let Some(resolved_swapchain) = &swapchain_image.resource_type {
                    if let ResourceType::Image(resolved_image) = resolved_swapchain {
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
                            .image(resolved_image.image)
                            .subresource_range(subresource_range)
                            .build()
                    } else {
                        panic!("Swapchain must be an image");
                    }
                } else {
                    panic!("Swapchain image should e valid for present");
                }
            };

            unsafe {
                self.render_context.get_device().borrow().get().cmd_pipeline_barrier(
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
            device.borrow().get().end_command_buffer(command_buffer)
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
            self.render_context.get_device().borrow().get()
                .reset_fences(&wait_fences)
                .expect("Failed to reset Fence!");

            // self.device
            self.render_context.get_device().borrow().get()
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
            let device = self.render_context.get_device();
            for i in 0..MAX_FRAMES_IN_FLIGHT {
                device.borrow().get().destroy_semaphore(self.image_available_semaphores[i], None);
                device.borrow().get().destroy_semaphore(self.render_finished_semaphores[i], None);
                device.borrow().get().destroy_fence(self.in_flight_fences[i], None);
            }

            for &framebuffer in self.swapchain_framebuffers.iter() {
                device.borrow().get().destroy_framebuffer(framebuffer, None);
            }

            device.borrow().get().destroy_render_pass(self.render_pass, None);

            device.borrow().get_debug_utils().destroy_debug_utils_messenger(
                self.debug_merssager,
                None);
        }
    }
}

// Fix content -------------------------------------------------------------------------------
impl VulkanApp {

    pub fn main_loop(mut self, event_loop: EventLoop<()>) {

        let mut mouse_pos: (f32, f32) = (0.0, 0.0);
        let mut mouse_down = false;

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
                        | WindowEvent::CursorMoved { position, .. } => {
                            mouse_pos.0 = position.x as f32;
                            mouse_pos.1 = position.y as f32;
                        },
                        | WindowEvent::MouseInput {button, state, .. } => {
                            if button == MouseButton::Left && state == ElementState::Pressed {
                                mouse_down = true;
                            }
                        }
                        | _ => {},
                    }
                },
                | Event::MainEventsCleared => {
                    self.window.request_redraw();
                },
                | Event::RedrawRequested(_window_id) => {
                    self.draw_frame(mouse_pos, mouse_down);
                },
                | Event::LoopDestroyed => {
                    unsafe {
                        self.render_context.get_device().borrow().get().device_wait_idle()
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
