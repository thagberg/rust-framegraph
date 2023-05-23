use std::cell::RefCell;
use std::rc::Rc;
use crate::api_types::device::{DeviceWrapper};

pub trait RenderContext  {
    type Create;
    type RP;

    fn create_renderpass(&self, create_info: &Self::Create) -> Self::RP;

    fn get_device(&self) -> Rc<RefCell<DeviceWrapper>>;
}

// pub trait CommandBuffer {
//
// }
