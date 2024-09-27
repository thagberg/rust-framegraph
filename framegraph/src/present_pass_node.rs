use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use api_types::device::DeviceResource;
use crate::pass_node::PassNode;

#[derive(Debug)]
pub struct PresentPassNode {
    pub swapchain_image: Arc<Mutex<DeviceResource>>,
    name: String
}

#[derive(Default)]
pub struct PresentPassNodeBuilder {
    name: String,
    swapchain_image: Option<Arc<Mutex<DeviceResource>>>
}

impl PresentPassNode {
    pub fn builder(name: String) -> PresentPassNodeBuilder {
        PresentPassNodeBuilder {
            name,
            ..Default::default()
        }
    }
}

impl PresentPassNodeBuilder {
    pub fn swapchain_image(mut self, swapchain_image: Arc<Mutex<DeviceResource>>) -> Self {
        self.swapchain_image = Some(swapchain_image);
        self
    }

    pub fn build(mut self) -> Result<PresentPassNode, &'static str> {
        if let Some(swapchain_image) = self.swapchain_image {
            Ok(PresentPassNode {
                swapchain_image,
                name: self.name
            })
        } else {
            Err("PresentPassNode requires a swapchain image")
        }
    }
}

impl PassNode for PresentPassNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_reads(&self) -> Vec<u64> {
        vec![self.swapchain_image.borrow().get_handle()]
    }

    fn get_writes(&self) -> Vec<u64> {
        vec![]
    }
}