use std::sync::{Arc, Mutex};
use imgui::Ui;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::interface::DeviceInterface;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;
use crate::example::Example;

pub struct DeferredExample {}

impl DeferredExample {
    pub fn new() -> Self {
        DeferredExample {}
    }
}

impl Example for DeferredExample {
    fn get_name(&self) -> &'static str {
        "Deferred"
    }

    fn execute(
        &self,
        _device: DeviceInterface,
        _allocator: Arc<Mutex<ResourceAllocator>>,
        imgui_ui: &mut Ui,
        _back_buffer: AttachmentReference) -> Vec<PassType> {
        
        imgui_ui.text("Deferred Shading Example (Stub)");
        
        Vec::new()
    }

    fn update(&mut self, _delta_time: f32) {}
}
