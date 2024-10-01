use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::DeviceWrapper;

#[derive(Clone)]
pub struct DevicePipeline {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pub device: Arc<Mutex<DeviceWrapper>>
}

impl Drop for DevicePipeline {
    fn drop(&mut self) {
        unsafe {
            let device_ref = self.device.lock()
                .expect("Failed to obtain device lock");
            device_ref.get().destroy_pipeline_layout(self.pipeline_layout, None);
            device_ref.get().destroy_pipeline(self.pipeline, None);
            for dsl in &self.descriptor_set_layouts {
                device_ref.get().destroy_descriptor_set_layout(*dsl, None);
            }
        }
    }
}

impl DevicePipeline {
    pub fn new(
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        device: Arc<Mutex<DeviceWrapper>>) -> Self {

        DevicePipeline {
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            device
        }
    }
}
