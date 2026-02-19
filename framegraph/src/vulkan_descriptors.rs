use ash::vk;
use crate::binding::{ResourceBinding, ImageBindingInfo, BufferBindingInfo, BindingType};
use crate::pipeline::Pipeline;
use api_types::image::ImageWrapper;
use api_types::buffer::BufferWrapper;
use api_types::device::resource::ResourceType;

pub struct DescriptorImageUpdate {
    pub image_info: vk::DescriptorImageInfo,
    pub descriptor_type: vk::DescriptorType,
    pub binding_set: vk::DescriptorSet,
    pub binding_slot: u32
}

pub struct DescriptorBufferUpdate {
    pub buffer_info: vk::DescriptorBufferInfo,
    pub descriptor_type: vk::DescriptorType,
    pub binding_set: vk::DescriptorSet,
    pub binding_slot: u32
}

/// Wrapper for all info required for vk::WriteDescriptorSet
/// This ensures that the image / buffer info references held in WriteDescriptorSet
/// will live long enough
pub struct DescriptorUpdate {
    pub image_infos: Vec<DescriptorImageUpdate>,
    pub buffer_infos: Vec<DescriptorBufferUpdate>
}

impl DescriptorUpdate {
    pub fn new() -> Self {
        DescriptorUpdate {
            image_infos: vec![],
            buffer_infos: vec![]
        }
    }

    pub fn create_descriptor_writes(&self) -> Vec<vk::WriteDescriptorSet>{
        let mut descriptor_writes = vec![];

        for image_info in &self.image_infos {
            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(image_info.binding_set)
                .dst_binding(image_info.binding_slot)
                .dst_array_element(0) // TODO: parameterize
                .descriptor_type(image_info.descriptor_type)
                .image_info(std::slice::from_ref(&image_info.image_info));

            descriptor_writes.push(descriptor_write);
        }

        for buffer_info in &self.buffer_infos {
            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(buffer_info.binding_set)
                .dst_binding(buffer_info.binding_slot)
                .dst_array_element(0) // TODO: parameterize
                .descriptor_type(buffer_info.descriptor_type)
                .buffer_info(std::slice::from_ref(&buffer_info.buffer_info));

            descriptor_writes.push(descriptor_write);
        }

        descriptor_writes
    }
}

pub fn get_descriptor_image_info(
    image: &ImageWrapper,
    binding_info: &ImageBindingInfo) -> (vk::DescriptorImageInfo, vk::DescriptorType) {

    let (sampler, descriptor_type) = match image.sampler {
        Some(s) => {(s, vk::DescriptorType::COMBINED_IMAGE_SAMPLER)}
        // None => {(vk::Sampler::null(), vk::DescriptorType::SAMPLED_IMAGE)}
        None => {(vk::Sampler::null(), vk::DescriptorType::STORAGE_IMAGE)}
    };
    let image_info = vk::DescriptorImageInfo::default()
        .image_view(image.view)
        .image_layout(binding_info.layout)
        .sampler(sampler);

    (image_info, descriptor_type)
}

pub fn get_descriptor_buffer_info(
    buffer: &BufferWrapper,
    binding: &BufferBindingInfo) -> (vk::DescriptorBufferInfo, vk::DescriptorType) {

    let buffer_info = vk::DescriptorBufferInfo::default()
        .buffer(buffer.buffer)
        .offset(binding.offset)
        .range(binding.range);
    let descriptor_type = vk::DescriptorType::UNIFORM_BUFFER; // TODO: this could also be a storage buffer

    (buffer_info, descriptor_type)
}

pub fn resolve_descriptors(
    bindings: &[ResourceBinding],
    _pipeline: &Pipeline,
    descriptor_sets: &[vk::DescriptorSet],
    descriptor_updates: &mut DescriptorUpdate) {

    for binding in bindings {
        let binding_ref = binding.resource.lock().unwrap();
        let resolved_binding = {
            binding_ref.resource_type.as_ref().expect("Invalid resource in binding")
        };
        let descriptor_set = descriptor_sets[binding.binding_info.set as usize];

        match (&resolved_binding, &binding.binding_info.binding_type) {
            (ResourceType::Image(resolved_image), BindingType::Image(image_binding)) => {
                let (image_info, descriptor_type) = get_descriptor_image_info(resolved_image, image_binding);
                descriptor_updates.image_infos.push(DescriptorImageUpdate{
                    image_info,
                    descriptor_type,
                    binding_set: descriptor_set,
                    binding_slot: binding.binding_info.slot
                });
            },
            (ResourceType::Buffer(resolved_buffer), BindingType::Buffer(buffer_binding)) => {
                let (buffer_info, descriptor_type) = get_descriptor_buffer_info(resolved_buffer, buffer_binding);
                descriptor_updates.buffer_infos.push(DescriptorBufferUpdate{
                    buffer_info,
                    descriptor_type,
                    binding_set: descriptor_set,
                    binding_slot: binding.binding_info.slot
                });
            },
            _ => {
                panic!("Invalid type being resolved");
            }
        }
    }
}
