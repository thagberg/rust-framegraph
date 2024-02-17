use imgui::Ui;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;
use crate::example::Example;

pub struct ModelExample {

}

impl Example for ModelExample {
    fn get_name(&self) -> &'static str {
        "Model Render"
    }

    fn execute(&self, imgui_ui: &mut Ui, back_buffer: AttachmentReference) -> Vec<PassType> {
        Vec::new()
    }
}