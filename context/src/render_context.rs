use std::sync::{Arc, Mutex};
use api_types::device::interface::DeviceInterface;

pub trait RenderContext  {
    type Create;
    type RP;

    fn get_device(&self) -> DeviceInterface;
}
