use imgui::Ui;
use gltf::Gltf;
use framegraph::attachment::AttachmentReference;
use framegraph::pass_type::PassType;
use crate::example::Example;

pub struct GltfModel {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>
}

pub struct ModelExample {
    duck_model: GltfModel
}

impl Example for ModelExample {
    fn get_name(&self) -> &'static str {
        "Model Render"
    }

    fn execute(&self, imgui_ui: &mut Ui, back_buffer: AttachmentReference) -> Vec<PassType> {
        Vec::new()
    }
}

impl ModelExample {
    pub fn new() -> Self {
        let duck_import = gltf::import("assets/models/gltf/duck/Duck.gltf");
        let duck_gltf = match duck_import {
            Ok(gltf) => {
                GltfModel {
                    document: gltf.0,
                    buffers: gltf.1,
                    images: gltf.2
                }
            },
            Err(e) => {
                panic!("Failed to open Duck gltf model: {}", e)
            }
        };

        ModelExample{
            duck_model: duck_gltf
        }
    }
}