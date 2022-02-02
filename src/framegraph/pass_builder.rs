struct PassBuilder {

}

impl PassBuilder {
    pub fn read(read_resource: ResourceHandle) -> ResourceHandle {
        let i: crate::framegraph::frame_graph::ResourceHandle = 1;
        read_resource
    }

    pub fn write(write_resource: ResourceHandle) -> ResourceHandle {
        write_resource + 1
    }

    pub fn create_texture(texture_desc: &TextureDesc) -> ResourceHandle {
        1
    }
}