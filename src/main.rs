use std::mem::swap;
use core::ffi::c_void;
use untitled::{
    utility, // the mod define some fixed functions that have been learned before.
    utility::constants::*,
    utility::debug::*,
    utility::share,
};

use ash::vk;
use winit::event::{Event, VirtualKeyCode, ElementState, KeyboardInput, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

use std::ptr;
use ash::extensions::khr::Surface;

mod context;
mod api_types;
mod resource;
mod framegraph;
use crate::context::render_context::RenderContext;
use crate::api_types::surface::SurfaceWrapper;
use crate::api_types::device::DeviceWrapper;
use crate::api_types::instance::InstanceWrapper;
use crate::framegraph::pass_node::{PassNodeBuilder, PassNode};

// Constants
const WINDOW_TITLE: &'static str = "15.Hello Triangle";
const MAX_FRAMES_IN_FLIGHT: usize = 2;

struct OffsetUBO {
    offset: [f32; 3]
}

struct SyncObjects {
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    inflight_fences: Vec<vk::Fence>,
}

struct VulkanApp {
    window: winit::window::Window,
    // vulkan stuff
    // _entry: ash::Entry,
    // instance: ash::Instance,
    // surface_loader: ash::extensions::khr::Surface,
    // surface: vk::SurfaceKHR,
    debug_utils_loader: ash::extensions::ext::DebugUtils,
    debug_merssager: vk::DebugUtilsMessengerEXT,

    // _physical_device: vk::PhysicalDevice,

    render_context: RenderContext,
    // device: ash::Device,

    // graphics_queue: vk::Queue,
    // present_queue: vk::Queue,

    // swapchain_loader: ash::extensions::khr::Swapchain,
    // swapchain: vk::SwapchainKHR,
    // _swapchain_images: Vec<vk::Image>,
    // _swapchain_format: vk::Format,
    // _swapchain_extent: vk::Extent2D,
    // swapchain_imageviews: Vec<vk::ImageView>,
    swapchain_framebuffers: Vec<vk::Framebuffer>,

    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    graphics_pipeline: vk::Pipeline,

    // command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

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
        // let surface_stuff =
        //     share::create_surface(&entry, &instance, &window, WINDOW_WIDTH, WINDOW_HEIGHT);
        let (debug_utils_loader, debug_merssager) =
            setup_debug_utils(VALIDATION.is_enable, &entry, &instance);

        let surface_wrapper = SurfaceWrapper::new(
            &entry,
            &instance,
            &window
        );

        let mut render_context = RenderContext::new(
            entry,
            instance,
            Some(surface_wrapper),
            &window);

        let uniform_buffer = render_context.create_uniform_buffer(
            std::mem::size_of::<OffsetUBO>() as vk::DeviceSize);
        let ubo_value = OffsetUBO {
            offset: [0.2, 0.1, 0.0]
        };
        render_context.update_uniform_buffer(&uniform_buffer, |mapped_memory: *mut c_void| {
            println!("Updating uniform buffer");
            unsafe {
                // *mapped_memory as OffsetUBO = ubo_value;
                core::ptr::copy_nonoverlapping(
                    &ubo_value,
                    mapped_memory as *mut OffsetUBO,
                    std::mem::size_of::<OffsetUBO>());
            };
        });

        let ubo_binding = vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::ALL_GRAPHICS,
            p_immutable_samplers: std::ptr::null()
        };
        let ubo_bindings = [ubo_binding];

        let ubo_descriptor_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 8
        };
        let descriptor_pool_sizes = [ubo_descriptor_size];
        let descriptor_pool_create = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::DescriptorPoolCreateFlags::empty(),
            max_sets: 8,
            pool_size_count: descriptor_pool_sizes.len() as u32,
            p_pool_sizes: descriptor_pool_sizes.as_ptr()
        };
        let descriptor_pool = unsafe {
            render_context.get_device().create_descriptor_pool(
                &descriptor_pool_create,
                None)
                .expect("Failed to create descriptor pool")
        };

        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            binding_count: ubo_bindings.len() as u32,
            p_bindings: ubo_bindings.as_ptr()
        };
        let descriptor_set_layout = unsafe {
            render_context.get_device().create_descriptor_set_layout(
                &descriptor_set_layout_create_info,
                None)
                .expect("Failed to create descriptor set layout")
        };
        let descriptor_set_layouts = [descriptor_set_layout];

        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
            p_next: ptr::null(),
            descriptor_pool,
            descriptor_set_count: descriptor_set_layouts.len() as u32,
            p_set_layouts: descriptor_set_layouts.as_ptr()
        };

        let descriptor_sets = unsafe {
            render_context.get_device().allocate_descriptor_sets(&descriptor_set_alloc_info)
                .expect("Failed to allocate descriptor sets")
        };

        let descriptor_buffer = vk::DescriptorBufferInfo {
            buffer: uniform_buffer.get(),
            offset: 0,
            range: std::mem::size_of::<OffsetUBO>() as vk::DeviceSize
        };

        let descriptor_write = vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            p_next: ptr::null(),
            dst_set: descriptor_sets[0],
            dst_binding: 0,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            p_buffer_info: &descriptor_buffer,
            ..Default::default()
        };
        let descriptor_write_sets = [descriptor_write];

        unsafe {
            render_context.get_device().update_descriptor_sets(&descriptor_write_sets, &[]);
        }

        assert!(render_context.get_swapchain().is_some(), "Can't continue without valid swapchain");
        let swapchain = &render_context.get_swapchain().as_ref().unwrap();
        assert!(render_context.get_swapchain_image_views().is_some(), "Can't continue without image views");
        let image_views = &render_context.get_swapchain_image_views().as_ref().unwrap();

        let render_pass = VulkanApp::create_render_pass(
            render_context.get_device(),
            swapchain.get_format());
        // swapchain_stuff.swapchain_format);
        let (graphics_pipeline, pipeline_layout) = share::v1::create_graphics_pipeline(
            render_context.get_device(),
            render_pass,
            swapchain.get_extent()
            // swapchain_stuff.swapchain_extent,
        );
        let swapchain_framebuffers = share::v1::create_framebuffers(
            render_context.get_device(),
            render_pass,
            image_views,
            // &swapchain_imageviews,
            swapchain.get_extent(),
        );

        // try creating a PassNode
        let pass_node = PassNode::builder()
            .renderpass(render_pass)
            .layout(pipeline_layout)
            .pipeline(graphics_pipeline)
            .build();


        // let command_pool = share::v1::create_command_pool(
        //     render_context.get_device(),
        //     render_context);
        let command_buffers = share::v1::create_command_buffers(
            render_context.get_device(),
            render_context.get_graphics_command_pool(),
            graphics_pipeline,
            &swapchain_framebuffers,
            render_pass,
            swapchain.get_extent(),
            &descriptor_sets,
            pipeline_layout
        );
        let sync_ojbects = VulkanApp::create_sync_objects(render_context.get_device());

        // cleanup(); the 'drop' function will take care of it.

        VulkanApp {
            window,
            // vulkan stuff
            // _entry: entry,
            // instance,
            // surface: surface_stuff.surface,
            // surface_loader: surface_stuff.surface_loader,
            debug_utils_loader,
            debug_merssager,

            // _physical_device: physical_device,
            render_context,
            // device,
            //
            // graphics_queue,
            // present_queue,

            // swapchain_loader: swapchain_stuff.swapchain_loader,
            // swapchain: swapchain_stuff.swapchain,
            // _swapchain_format: swapchain_stuff.swapchain_format,
            // _swapchain_images: swapchain_stuff.swapchain_images,
            // _swapchain_extent: swapchain_stuff.swapchain_extent,
            // swapchain_imageviews,
            swapchain_framebuffers,

            pipeline_layout,
            render_pass,
            graphics_pipeline,

            // command_pool,
            command_buffers,

            image_available_semaphores: sync_ojbects.image_available_semaphores,
            render_finished_semaphores: sync_ojbects.render_finished_semaphores,
            in_flight_fences: sync_ojbects.inflight_fences,
            current_frame: 0,
        }
    }

    fn draw_frame(&mut self) {
        let wait_fences = [self.in_flight_fences[self.current_frame]];

        let (image_index, _is_sub_optimal) = unsafe {
            // self.device
            self.render_context.get_device()
                .wait_for_fences(&wait_fences, true, std::u64::MAX)
                .expect("Failed to wait for Fence!");

            self.render_context.get_swapchain().as_ref().unwrap().get_loader()
            // self.swapchain_loader
                .acquire_next_image(
                    // self.swapchain,
                    self.render_context.get_swapchain().as_ref().unwrap().get(),
                    std::u64::MAX,
                    self.image_available_semaphores[self.current_frame],
                    vk::Fence::null(),
                )
                .expect("Failed to acquire next image.")
        };

        let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished_semaphores[self.current_frame]];

        let submit_infos = [vk::SubmitInfo {
            s_type: vk::StructureType::SUBMIT_INFO,
            p_next: ptr::null(),
            wait_semaphore_count: wait_semaphores.len() as u32,
            p_wait_semaphores: wait_semaphores.as_ptr(),
            p_wait_dst_stage_mask: wait_stages.as_ptr(),
            command_buffer_count: 1,
            p_command_buffers: &self.command_buffers[image_index as usize],
            signal_semaphore_count: signal_semaphores.len() as u32,
            p_signal_semaphores: signal_semaphores.as_ptr(),
        }];

        unsafe {
            // self.device
            self.render_context.get_device()
                .reset_fences(&wait_fences)
                .expect("Failed to reset Fence!");

            // self.device
            self.render_context.get_device()
                .queue_submit(
                    // self.graphics_queue,
                    self.render_context.get_graphics_queue(),
                    &submit_infos,
                    self.in_flight_fences[self.current_frame],
                )
                .expect("Failed to execute queue submit.");
        }

        let swapchains = [self.render_context.get_swapchain().as_ref().unwrap().get()];

        let present_info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            p_next: ptr::null(),
            wait_semaphore_count: 1,
            p_wait_semaphores: signal_semaphores.as_ptr(),
            swapchain_count: 1,
            p_swapchains: swapchains.as_ptr(),
            p_image_indices: &image_index,
            p_results: ptr::null_mut(),
        };

        unsafe {
            // self.swapchain_loader
            self.render_context.get_swapchain().as_ref().unwrap().get_loader()
                // .queue_present(self.present_queue, &present_info)
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
                device.destroy_semaphore(self.image_available_semaphores[i], None);
                device.destroy_semaphore(self.render_finished_semaphores[i], None);
                device.destroy_fence(self.in_flight_fences[i], None);
            }

            // device.destroy_command_pool(self.command_pool, None);

            for &framebuffer in self.swapchain_framebuffers.iter() {
                device.destroy_framebuffer(framebuffer, None);
            }

            device.destroy_pipeline(self.graphics_pipeline, None);
           device .destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_render_pass(self.render_pass, None);

            // for &imageview in self.swapchain_imageviews.iter() {
            //     device.destroy_image_view(imageview, None);
            // }

            // self.swapchain_loader
            //     .destroy_swapchain(self.swapchain, None);
            // device.destroy_device(None);
            // self.surface_loader.destroy_surface(self.surface, None);

            if VALIDATION.is_enable {
                self.debug_utils_loader
                    .destroy_debug_utils_messenger(self.debug_merssager, None);
            }
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
                        self.render_context.get_device().device_wait_idle()
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
