use std::sync::{Mutex};
use ash::vk;
use tracy_client::{GpuContext, GpuContextType, GpuSpan};

struct ClosedGpuSpan {
    span: Option<GpuSpan>,
    start_query_id: u32,
    end_query_id: u32
}

impl ClosedGpuSpan {
    fn new(span: Option<GpuSpan>, start_query_id: u32, end_query_id: u32) -> Self {
        ClosedGpuSpan{
            span,
            start_query_id,
            end_query_id,
        }
    }
}

pub struct OpenGpuSpan<'a> {
    query_id: u32,
    device: &'a ash::Device,
    command_buffer: &'a vk::CommandBuffer,
    pipeline_stage: vk::PipelineStageFlags,
    // the span is not actually optional, but this gives us something that
    // implements Default so we can use std::mem::take on it to move the
    // GpuSpan out on Drop
    span: Option<GpuSpan>
}

impl Drop for OpenGpuSpan<'_> {
    fn drop(&mut self) {
        let mut span_mutex = GPU_SPAN_MANAGER.lock().unwrap();
        match span_mutex.as_mut() {
            None => {
                panic!("Attempting to close GPU span before GpuSpanManager was initialized")
            }
            Some(span_manager) => {
                span_manager.close_gpu_span(
                    std::mem::take(&mut self.span),
                    self.query_id,
                    self.command_buffer,
                    self.device,
                    self.pipeline_stage);
            }
        }
    }
}

impl<'a> OpenGpuSpan<'a> {
    fn new(
        span: GpuSpan,
        query_id: u32,
        device: &'a ash::Device,
        command_buffer: &'a vk::CommandBuffer,
        pipeline_stage: vk::PipelineStageFlags) -> Self {

        OpenGpuSpan {
            query_id,
            device,
            command_buffer,
            pipeline_stage,
            span: Some(span)
        }
    }
}

const MAX_QUERIES: u32 = 128;

struct FrameSpans {
    query_pool: vk::QueryPool,
    active_spans: Vec<ClosedGpuSpan>,
    max_queries: u32,
    query_index: u32,
    ready: bool,
    data: [i64; MAX_QUERIES as usize]
}

impl FrameSpans {
    pub fn reset(&mut self, device: &ash::Device) {
        unsafe {
            device.reset_query_pool(
                self.query_pool,
                0,
                self.query_index+1
            );
        }
        self.query_index = 0;
        self.active_spans.clear();
        self.ready = true;
    }

    pub fn flush(&mut self, device: &ash::Device) {
        // if query_index is still 0, we haven't written a query yet
        if self.query_index > 0 {
            unsafe {
                device.get_query_pool_results(
                    self.query_pool,
                    0, // should this be query_index?
                    &mut self.data[0..self.query_index as usize],
                    vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT)
                    .expect("Failed to retrieve query results");
            }

            for active_span in &mut self.active_spans {
                let start_timestamp = self.data[active_span.start_query_id as usize];
                let end_timestamp = self.data[active_span.end_query_id as usize];

                let mut gpu_span = None;
                std::mem::swap(&mut gpu_span, &mut active_span.span);

                match gpu_span {
                    None => {
                        panic!("Attempting to upload an invalid GPU span");
                    }
                    Some(span) => {
                        span.upload_timestamp(start_timestamp as i64, end_timestamp as i64);
                    }
                }
            }
        }
        self.ready = false;
    }

    pub fn new_gpu_span<'a>(
        &mut self,
        name: &str,
        file: &str,
        function: &str,
        line_number: u32,
        gpu_context: &GpuContext,
        device: &'a ash::Device,
        command_buffer: &'a vk::CommandBuffer,
        pipeline_stage: vk::PipelineStageFlags) -> OpenGpuSpan<'a> {

        assert!(self.ready, "Attempting to create GPU span before resetting the query pool");
        assert!(self.query_index < self.max_queries, "Overallocating GPU timespan queries");

        let new_span = gpu_context.span_alloc(name, function, file, line_number)
            .expect("Failed to create new GPU span");

        let query_index = self.query_index;

        unsafe {
            device.cmd_write_timestamp(
                *command_buffer,
                pipeline_stage,
                self.query_pool.clone(),
                query_index
            );
        }

        self.query_index += 1;
        OpenGpuSpan::new(
            new_span,
            query_index,
            device,
            command_buffer,
            pipeline_stage)
    }

    pub fn close_gpu_span(
        &mut self,
        mut span: Option<GpuSpan>,
        start_query_id: u32,
        command_buffer: &vk::CommandBuffer,
        device: &ash::Device,
        pipeline_stage: vk::PipelineStageFlags) {

        assert!(self.ready, "Attempting to close GPU span before resetting the query pool");
        assert!(self.query_index < self.max_queries, "Overallocating GPU timespan queries");

        span.as_mut().unwrap().end_zone();

        unsafe {
            device.cmd_write_timestamp(
                *command_buffer,
                pipeline_stage,
                self.query_pool.clone(),
                self.query_index
            );
        }

        self.active_spans.push(ClosedGpuSpan::new(
            span,
            start_query_id,
             self.query_index,
        ));

        self.query_index += 1;

    }
}

pub struct GpuSpanManager {
    frames: Vec<FrameSpans>,
    frame_index: usize,
    gpu_context: GpuContext
}

static GPU_SPAN_MANAGER: Mutex<Option<GpuSpanManager>> = Mutex::new(None);

impl GpuSpanManager {
    pub fn init(
        device: &ash::Device,
        timestamp_period: f32,
        command_buffer: &vk::CommandBuffer,
        queue: &vk::Queue,
        num_frames: u32) {

        unsafe {
            assert!(GPU_SPAN_MANAGER.lock().unwrap().is_none(), "Can only initialize a single GpuSpanManagera");

            let mut frames: Vec<FrameSpans> = Vec::new();

            let query_pool_create = vk::QueryPoolCreateInfo::default()
                .query_type(vk::QueryType::TIMESTAMP)
                .query_count(MAX_QUERIES);

            for _i in 0..num_frames {
                let query_pool = device.create_query_pool(
                    &query_pool_create,
                    None
                ).expect("Failed to create query pool");

                frames.push(FrameSpans {
                    query_pool,
                    active_spans: vec![],
                    max_queries: MAX_QUERIES,
                    query_index: 0,
                    ready: false,
                    data: [0; MAX_QUERIES as usize],
                })
            }

            // query pools must be reset before they can be used
            for frame in &frames {
                device.reset_query_pool(
                    frame.query_pool,
                    0,
                    frame.max_queries
                );
            }

            // initial timestamp query
            let mut timestamp_value: i64 = 0;

            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            device.begin_command_buffer(*command_buffer, &begin_info)
                .expect("Failed to begin command buffer for profiling");

            device.cmd_write_timestamp(
                *command_buffer,
                vk::PipelineStageFlags::ALL_GRAPHICS,
                frames[0].query_pool.clone(),
                0
            );

            device.end_command_buffer(*command_buffer)
                .expect("Failed to end command buffer for profiling");

            let submit_info = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(command_buffer));

            device.queue_submit(
                queue.clone(),
                std::slice::from_ref(&submit_info),
                vk::Fence::null()
            ).expect("Failed to submit queue for profiling");

            device.device_wait_idle()
                .expect("Failed to wait for idle for profiling");

            device.get_query_pool_results(
                frames[0].query_pool,
                0,
                std::slice::from_mut(&mut timestamp_value),
                vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT
            ).expect("Failed to retrieve initial GPU timestamp");

            let tc = tracy_client::Client::start();
            let gpu_context = tc.new_gpu_context(
                Some("VulkanContext"),
                GpuContextType::Vulkan,
                timestamp_value as i64,
                timestamp_period)
                .expect("Failed to create GPU profiling context");

            *GPU_SPAN_MANAGER.lock().unwrap() = Some(GpuSpanManager {
                frames,
                frame_index: 0,
                gpu_context
            });

        }
    }

    fn reset(&mut self, device: &ash::Device) {
        self.frame_index = (self.frame_index + 1) % self.frames.len();
        match self.frames.get_mut(self.frame_index) {
            None => {
                panic!("Attempting to reset GpuSpanManager frame with invalid index");
            }
            Some(frame) => {
                frame.flush(device);
                frame.reset(device);
            }
        }
    }

    fn _flush(&mut self, device: &ash::Device) {
        match self.frames.get_mut(self.frame_index) {
            None => {
                panic!("Attempting to flush GpuSpanManager frame with invalid index");
            }
            Some(frame) => {
                frame.flush(device);
            }
        }
    }

    fn new_gpu_span<'a>(
        &mut self,
        name: &str,
        file: &str,
        function: &str,
        line_number: u32,
        device: &'a ash::Device,
        command_buffer: &'a vk::CommandBuffer,
        pipeline_stage: vk::PipelineStageFlags) -> OpenGpuSpan<'a> {

        match self.frames.get_mut(self.frame_index) {
            None => {
                panic!("Attempting to flush GpuSpanManager frame with invalid index");
            }
            Some(frame) => {
                frame.new_gpu_span(name, file, function, line_number, &self.gpu_context, device, command_buffer, pipeline_stage)
            }
        }
    }

    fn close_gpu_span(
        &mut self,
        span: Option<GpuSpan>,
        start_query_id: u32,
        command_buffer: &vk::CommandBuffer,
        device: &ash::Device,
        pipeline_stage: vk::PipelineStageFlags) {

        match self.frames.get_mut(self.frame_index) {
            None => {
                panic!("Attempting to flush GpuSpanManager frame with invalid index");
            }
            Some(frame) => {
                frame.close_gpu_span(span, start_query_id, command_buffer, device, pipeline_stage);
            }
        }
    }
}

pub fn reset_span_manager(device: &ash::Device) {
    let mut span_mutex = GPU_SPAN_MANAGER.lock().unwrap();
    match span_mutex.as_mut() {
        None => {
            panic!("Attempting to enter GPU span before GpuSpanManager was initialized")
        }
        Some(span_manager) => {
            span_manager.reset(device);
        }
    }
}

pub fn new_gpu_span<'a>(
    name: &str,
    file: &str,
    function: &str,
    line_number: u32,
    device: &'a ash::Device,
    command_buffer: &'a vk::CommandBuffer,
    pipeline_stage: vk::PipelineStageFlags) -> OpenGpuSpan<'a> {

    let mut span_mutex = GPU_SPAN_MANAGER.lock().unwrap();
    match span_mutex.as_mut() {
        None => {
            panic!("Attempting to enter GPU span before GpuSpanManager was initialized")
        }
        Some(span_manager) => {
            span_manager.new_gpu_span(name, file, function, line_number, device, command_buffer, pipeline_stage)
        }
    }
}

#[macro_export]
macro_rules! init_gpu_profiling {
    ($device:expr, $period:expr, $cb:expr, $queue:expr, $num_frames:expr) => {
        profiling::GpuSpanManager::init($device, $period, $cb, $queue, $num_frames);
    }
}

#[macro_export]
macro_rules! reset_gpu_profiling {
    ($device:expr) => {
        profiling::reset_span_manager($device);
    }
}

#[macro_export]
macro_rules! enter_gpu_span {
    ($name:expr, $function:expr, $device:expr, $command_buffer:expr, $pipeline_stage:expr) => {
        let _gpu_span = profiling::new_gpu_span($name, file!(), $function, line!(), $device, $command_buffer, $pipeline_stage);
    }
}

// https://docs.vulkan.org/spec/latest/chapters/queries.html#queries-timestamps
// GPU span struct
// command buffer arg and store reference / copy
// devicewrapper arg
// mutable reference to start and end timestamps
// on new, add timestamp query to command buffer
// on drop, add timestamp query to command buffer

#[macro_export]
macro_rules! enter_span {
    ($level:expr, $name:expr, $($fields:tt)*) => {
        let span = tracing::span!($level, $name, $($fields)*);
        let _enter = span.enter();
    };

    ($level:expr, $name:expr) => {
        enter_span!($level, $name,)
    };
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
