use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use ash::vk::CommandBuffer;
use api_types::device::DeviceResource;
use context::vulkan_render_context::VulkanRenderContext;
use crate::pass_node::{FillCallback, PassNode};

pub struct CopyPassNode {
    pub copy_sources: Vec<Arc<Mutex<DeviceResource>>>,
    pub copy_dests: Vec<Arc<Mutex<DeviceResource>>>,
    pub fill_callback: Box<FillCallback>,
    name: String
}

impl Debug for CopyPassNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyPassNode")
            .field("name", &self.name)
            .field("copy sources", &self.copy_sources)
            .field("copy dests", &self.copy_dests)
            .finish()
    }
}

impl CopyPassNode {
    pub fn builder(name: String) -> CopyPassNodeBuilder {
        CopyPassNodeBuilder {
            name,
            ..Default::default()
        }
    }

    pub fn execute(&self, render_context: &mut VulkanRenderContext, command_buffer: &CommandBuffer) {
        (self.fill_callback)(render_context, command_buffer);
    }
}

impl PassNode for CopyPassNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_reads(&self) -> Vec<u64> {
        let mut reads: Vec<u64> = Vec::new();
        reads.reserve(self.copy_sources.len());
        for source in &self.copy_sources {
            reads.push(source.lock().unwrap().get_handle());
        }

        reads
    }

    fn get_writes(&self) -> Vec<u64> {
        let mut writes: Vec<u64> = Vec::new();
        writes.reserve(self.copy_dests.len());
        for dest in &self.copy_dests {
            writes.push(dest.lock().unwrap().get_handle());
        }

        writes
    }
}

#[derive(Default)]
pub struct CopyPassNodeBuilder {
    copy_sources: Vec<Arc<Mutex<DeviceResource>>>,
    copy_dests: Vec<Arc<Mutex<DeviceResource>>>,
    fill_callback: Option<Box<FillCallback>>,
    name: String
}

impl CopyPassNodeBuilder {
    pub fn copy_src(mut self, copy_src: Arc<Mutex<DeviceResource>>) -> Self {
        self.copy_sources.push(copy_src);
        self
    }

    pub fn copy_dst(mut self, copy_dst: Arc<Mutex<DeviceResource>>) -> Self {
        self.copy_dests.push(copy_dst);
        self
    }

    pub fn fill_commands(mut self, fill_callback: Box<FillCallback>) -> Self
    {
        self.fill_callback = Some(fill_callback);
        self
    }

    pub fn build(mut self) -> Result<CopyPassNode, &'static str> {
        if let Some(_) = &self.fill_callback {
            let copy_sources_len = self.copy_sources.len();
            let copy_dests_len = self.copy_dests.len();

            Ok(CopyPassNode {
                copy_sources: self.copy_sources.into_iter().take(copy_sources_len).collect(),
                copy_dests: self.copy_dests.into_iter().take(copy_dests_len).collect(),
                fill_callback: self.fill_callback.take().unwrap(),
                name: self.name
            })
        } else {
            Err("CopyPassNodeBuilder was incomplete before building")
        }
    }
}