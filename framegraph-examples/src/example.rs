use alloc::rc::Rc;
use std::cell::RefCell;
use imgui::Ui;
use context::api_types::device::DeviceWrapper;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;

pub trait Example {
    fn get_name(&self) -> &'static str;

    fn execute(&self, device: Rc<RefCell<DeviceWrapper>>, imgui_ui: &mut Ui, back_buffer: AttachmentReference) -> Vec<PassType>;
}