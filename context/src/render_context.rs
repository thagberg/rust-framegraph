use std::cell::RefCell;
use std::sync::Arc;
use api_types::device::DeviceWrapper;

pub trait RenderContext  {
    type Create;
    type RP;

    fn get_device(&self) -> Arc<RefCell<DeviceWrapper>>;
}
