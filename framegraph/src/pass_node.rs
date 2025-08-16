use std::fmt::{Debug};
use std::sync::{Arc, Mutex};
use api_types::device::interface::DeviceInterface;
use ash::vk;

pub type FillCallback<'a> = dyn (
Fn(
    &DeviceInterface,
    vk::CommandBuffer
)
) + Sync + Send + 'a;

pub trait PassNode<'d> {
    fn get_name(&self) -> &str;

    fn get_reads(&self) -> Vec<u64>;

    fn get_writes(&self) -> Vec<u64>;
}