use std::cell::RefCell;
use std::ops::DerefMut;
use std::rc::Rc;
use api_types::device::DeviceWrapper;

/// Context for creating gpu spans.
///
/// Generally corresponds to a single hardware queue.
///
/// The flow of creating and using gpu context generally looks like this:
///
/// ```rust,no_run
/// # let client = tracy_client::Client::start();
/// // The period of the gpu clock in nanoseconds, as provided by your GPU api.
/// // This value corresponds to 1GHz.
/// let period: f32 = 1_000_000_000.0;
///
/// // GPU API: Record writing a timestamp and resolve that to a mappable buffer.
/// // GPU API: Submit the command buffer writing the timestamp.
/// // GPU API: Immediately block until the submission is finished.
/// // GPU API: Map buffer, get timestamp value.
/// let starting_timestamp: i64 = /* whatever you get from this timestamp */ 0;
///
/// // Create the gpu context
/// let gpu_context = client.new_gpu_context(
///     Some("MyContext"),
///     tracy_client::GpuContextType::Vulkan,
///     starting_timestamp,
///     period
/// ).unwrap();
///
/// // Now you have some work that you want to time on the gpu.
///
/// // GPU API: Record writing a timestamp before the work.
/// let mut span = gpu_context.span_alloc("MyGpuSpan1", "My::Work", "myfile.rs", 12).unwrap();
///
/// // GPU API: Record work.
///
/// // GPU API: Record writing a timestamp after the work.
/// span.end_zone();
///
/// // Some time later, once the written timestamp values are available on the cpu.
/// # let (starting_timestamp, ending_timestamp) = (0, 0);
///
/// // Consumes span.
/// span.upload_timestamp(starting_timestamp, ending_timestamp);
///

// function for getting initial timestamp (I believe this is used for timeline synchronization?)
pub fn get_gpu_timestamp(device: Rc<RefCell<DeviceWrapper>>) {
    unsafe {
        device.borrow().get().cmd_write_timestamp(Default::default(), Default::default(), Default::default(), 0)
    }
}

use std::sync::{LockResult, Mutex, MutexGuard};
use ash::vk;

struct ClosedGpuSpan {
    start_query_id: u32,
    end_query_id: u32
}

impl ClosedGpuSpan {
    fn new(start_query_id: u32, end_query_id: u32) -> Self {
        ClosedGpuSpan{
            start_query_id,
            end_query_id,
        }
    }
}

pub struct OpenGpuSpan<'a> {
    query_id: u32,
    device: &'a ash::Device,
    command_buffer: &'a vk::CommandBuffer,
    pipeline_stage: vk::PipelineStageFlags
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
        query_id: u32,
        device: &'a ash::Device,
        command_buffer: &'a vk::CommandBuffer,
        pipeline_stage: vk::PipelineStageFlags) -> Self {
        OpenGpuSpan {
            query_id,
            device,
            command_buffer,
            pipeline_stage
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
    data: [u64; MAX_QUERIES as usize]
}

impl FrameSpans {
    pub fn reset(&mut self, device: &ash::Device) {
        self.query_index = 0;
        unsafe {
            device.reset_query_pool(
                self.query_pool,
                0,
                self.max_queries-1
            );
        }
        self.ready = true;
    }

    pub fn flush(&mut self, device: &ash::Device) {
        // if query_index is still 0, we haven't written a query yet
        if (self.query_index > 0) {
            unsafe {
                device.get_query_pool_results(
                    self.query_pool,
                    0,
                    self.query_index,
                    &mut self.data,
                    vk::QueryResultFlags::TYPE_64 | vk::QueryResultFlags::WAIT)
                    .expect("Failed to retrieve query results");
            }
        }
        self.ready = false;
    }

    pub fn new_gpu_span<'a>(
        &mut self,
        device: &'a ash::Device,
        command_buffer: &'a vk::CommandBuffer,
        pipeline_stage: vk::PipelineStageFlags) -> OpenGpuSpan<'a> {

        assert!(self.ready, "Attempting to create GPU span before resetting the query pool");
        assert!(self.query_index < self.max_queries, "Overallocating GPU timespan queries");

        unsafe {
            device.cmd_write_timestamp(
                *command_buffer,
                pipeline_stage,
                self.query_pool.clone(),
                self.query_index
            );
        }

        self.query_index += 1;
        OpenGpuSpan::new(self.query_index, device, command_buffer, pipeline_stage)
    }

    pub fn close_gpu_span(
        &mut self,
        start_query_id: u32,
        command_buffer: &vk::CommandBuffer,
        device: &ash::Device,
        pipeline_stage: vk::PipelineStageFlags) {

        assert!(self.ready, "Attempting to close GPU span before resetting the query pool");
        assert!(self.query_index < self.max_queries, "Overallocating GPU timespan queries");

        unsafe {
            device.cmd_write_timestamp(
                *command_buffer,
                pipeline_stage,
                self.query_pool.clone(),
                self.query_index
            );
        }

        self.active_spans.push(ClosedGpuSpan {
            start_query_id,
            end_query_id: self.query_index,
        });
        self.query_index += 1;

    }
}

pub struct GpuSpanManager {
    frames: Vec<FrameSpans>,
    frame_index: usize
}

static GPU_SPAN_MANAGER: Mutex<Option<GpuSpanManager>> = Mutex::new(None);

impl GpuSpanManager {
    pub fn init(device: &ash::Device, num_frames: u32) {
        unsafe {
            assert!(GPU_SPAN_MANAGER.lock().unwrap().is_none(), "Can only initialize a single GpuSpanManagera");

            let mut frames: Vec<FrameSpans> = Vec::new();

            let query_pool_create = vk::QueryPoolCreateInfo::builder()
                .query_type(vk::QueryType::TIMESTAMP)
                .query_count(MAX_QUERIES)
                .build();

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

            *GPU_SPAN_MANAGER.lock().unwrap() = Some(GpuSpanManager {
                frames,
                frame_index: 0
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

    fn flush(&mut self, device: &ash::Device) {
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
        device: &'a ash::Device,
        command_buffer: &'a vk::CommandBuffer,
        pipeline_stage: vk::PipelineStageFlags) -> OpenGpuSpan<'a> {

        match self.frames.get_mut(self.frame_index) {
            None => {
                panic!("Attempting to flush GpuSpanManager frame with invalid index");
            }
            Some(frame) => {
                frame.new_gpu_span(device, command_buffer, pipeline_stage)
            }
        }
    }

    fn close_gpu_span(
        &mut self,
        start_query_id: u32,
        command_buffer: &vk::CommandBuffer,
        device: &ash::Device,
        pipeline_stage: vk::PipelineStageFlags) {

        match self.frames.get_mut(self.frame_index) {
            None => {
                panic!("Attempting to flush GpuSpanManager frame with invalid index");
            }
            Some(frame) => {
                frame.close_gpu_span(start_query_id, command_buffer, device, pipeline_stage);
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
    device: &'a ash::Device,
    command_buffer: &'a vk::CommandBuffer,
    pipeline_stage: vk::PipelineStageFlags) -> OpenGpuSpan<'a> {

    let mut span_mutex = GPU_SPAN_MANAGER.lock().unwrap();
    match span_mutex.as_mut() {
        None => {
            panic!("Attempting to enter GPU span before GpuSpanManager was initialized")
        }
        Some(span_manager) => {
            span_manager.new_gpu_span(device, command_buffer, pipeline_stage)
        }
    }
}

#[macro_export]
macro_rules! init_gpu_profiling {
    ($device:expr, $num_frames:expr) => {
        profiling::GpuSpanManager::init($device, $num_frames);
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
    ($device:expr, $command_buffer:expr, $pipeline_stage:expr) => {
        let _gpu_span = profiling::new_gpu_span($device, $command_buffer, $pipeline_stage);
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
