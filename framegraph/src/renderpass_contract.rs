use ash::vk;

pub struct NodeAttachmentDescription {
    pub format: vk::Format,
    pub initial_layout: vk::ImageLayout,
    pub final_layout: vk::ImageLayout
}

impl NodeAttachmentDescription {
    pub fn new(
        format: vk::Format,
        initial_layout: vk::ImageLayout,
        final_layout: vk::ImageLayout) -> NodeAttachmentDescription {

        NodeAttachmentDescription {
            format,
            initial_layout,
            final_layout
        }
    }
}

pub struct RenderpassContract {
    attachments: Vec<NodeAttachmentDescription>
}

impl RenderpassContract {
    pub fn new(attachments: Vec<NodeAttachmentDescription>) -> RenderpassContract {
        RenderpassContract {
            attachments
        }
    }
}

pub trait NodeContract {
    fn get_contract(&self) -> &RenderpassContract;
}