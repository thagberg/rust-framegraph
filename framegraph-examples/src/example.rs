use alloc::rc::Rc;
use std::cell::RefCell;
use imgui::Ui;
use api_types::device::interface::DeviceInterface;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;

pub trait Example<'d> {
    fn get_name(&self) -> &'static str;

    fn execute(
        &self,
        device: &'d DeviceInterface,
        imgui_ui: &mut Ui,
        back_buffer: AttachmentReference<'d>) -> Vec<PassType<'d>>;
}