use api_types::device::interface::DeviceInterface;
use ash::vk;

pub type FillCallback = dyn (
Fn(
    DeviceInterface,
    vk::CommandBuffer
)
) + Sync + Send;

pub trait PassNode {
    fn get_name(&self) -> &str;

    fn get_reads(&self) -> Vec<u64>;

    fn get_writes(&self) -> Vec<u64>;
}