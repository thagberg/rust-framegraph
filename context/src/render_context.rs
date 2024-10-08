use std::cell::RefCell;
use std::rc::Rc;
use api_types::device::DeviceWrapper;

pub trait RenderContext  {
    type Create;
    type RP;

    fn get_device(&self) -> Rc<RefCell<DeviceWrapper>>;
}
