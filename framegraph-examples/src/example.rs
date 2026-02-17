use alloc::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use imgui::Ui;
use api_types::device::allocator::ResourceAllocator;
use api_types::device::interface::DeviceInterface;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;

pub trait Example {
    fn get_name(&self) -> &'static str;

    fn execute(
        &self,
        device: DeviceInterface,
        allocator: Arc<Mutex<ResourceAllocator>>,
        imgui_ui: &mut Ui,
        back_buffer: AttachmentReference) -> Vec<PassType>;

    fn update(&mut self, _delta_time: f32) {}
}