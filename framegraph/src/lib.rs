pub mod frame_graph;
pub mod pass_node;
pub mod resource;
pub mod pipeline;
pub mod shader;

#[cfg(test)]
mod tests
{
    use crate::resource::i_resource_manager::ResourceManager;

    struct MockResourceManager {

    }

    impl ResourceManager for MockResourceManager {

    }

    #[test]
    fn dummy_test() {
        println!("Running a test");
        assert_eq!(1, 1);
    }

    #[test]
}
