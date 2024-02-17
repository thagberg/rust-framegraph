use imgui::Ui;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;

pub trait Example {
    fn get_name(&self) -> &'static str;

    fn execute(&self, imgui_ui: &mut Ui,back_buffer: AttachmentReference) -> Vec<PassType>;
}