use std::fmt::Debug;
use crate::resource::resource_manager::ResourceHandle;

pub trait PassNode {
    fn get_name(&self) -> &str;

    fn get_inputs(&self) -> &[ResourceHandle];

    fn get_outputs(&self) -> &[ResourceHandle];

    fn get_rendertargets(&self) -> &[ResourceHandle];
}