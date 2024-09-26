use std::sync::{Arc, Mutex};
use api_types::device::DeviceWrapper;

pub trait RenderContext  {
    type Create;
    type RP;

    fn get_device(&self) -> Arc<Mutex<DeviceWrapper>>;
}
