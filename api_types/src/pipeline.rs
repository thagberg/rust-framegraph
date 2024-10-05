use std::sync::{Arc, Mutex};
use ash::vk;
use crate::device::interface::DeviceInterface;

#[derive(Clone)]
pub struct DevicePipeline<'a> {
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    pub device: &'a DeviceInterface
}

impl Drop for DevicePipeline<'_> {
    fn drop(&mut self) {
        unsafe {
            self.device.get().destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.get().destroy_pipeline(self.pipeline, None);
            for dsl in &self.descriptor_set_layouts {
                self.device.get().destroy_descriptor_set_layout(*dsl, None);
            }
        }
    }
}

impl<'a> DevicePipeline<'a> {
    pub fn new(
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
        device: &'a DeviceInterface) -> Self {

        DevicePipeline {
            pipeline,
            pipeline_layout,
            descriptor_set_layouts,
            device
        }
    }
}
