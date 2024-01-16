use imgui::Ui;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;

pub trait Example {
    fn get_name(&self) -> &'static str;

    fn set_active(&mut self, active: bool);

    fn get_active(&self) -> bool;

    fn execute(&self, imgui_ui: &Ui, back_buffer: AttachmentReference) -> Vec<PassType>;
}