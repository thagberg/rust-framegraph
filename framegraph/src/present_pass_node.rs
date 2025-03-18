use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use api_types::device::resource::DeviceResource;
use crate::pass_node::PassNode;

#[derive(Debug)]
pub struct PresentPassNode<'a> {
    pub swapchain_image: Arc<Mutex<DeviceResource<'a>>>,
    name: String
}

#[derive(Default)]
pub struct PresentPassNodeBuilder<'a> {
    name: String,
    swapchain_image: Option<Arc<Mutex<DeviceResource<'a>>>>
}

impl<'a> PresentPassNode<'a> {
    pub fn builder(name: String) -> PresentPassNodeBuilder<'a> {
        PresentPassNodeBuilder {
            name,
            ..Default::default()
        }
    }
}

impl<'a> PresentPassNodeBuilder<'a> {
    pub fn swapchain_image(mut self, swapchain_image: Arc<Mutex<DeviceResource<'a>>>) -> Self {
        self.swapchain_image = Some(swapchain_image);
        self
    }

    pub fn build(mut self) -> Result<PresentPassNode<'a>, &'static str> {
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

impl<'d> PassNode<'d> for PresentPassNode<'d> {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_reads(&self) -> Vec<u64> {
        vec![self.swapchain_image.lock().unwrap().get_handle()]
    }

    fn get_writes(&self) -> Vec<u64> {
        vec![]
    }
}