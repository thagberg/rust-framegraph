pub mod frame_graph;
pub mod pass_node;
pub mod resource;
pub mod pipeline;
pub mod shader;
mod i_pass_node;

#[cfg(test)]
mod tests
{
    use crate::resource::i_resource_manager::ResourceManager;
    use crate::resource::resource_manager::ResourceHandle;
    use crate::i_pass_node::PassNode;
    use crate::frame_graph::FrameGraph;

    struct MockResourceManager {

    }

    impl ResourceManager for MockResourceManager {

    }

    struct MockPassNode {
        name: String,
        inputs: Vec<ResourceHandle>,
        outputs: Vec<ResourceHandle>,
        render_targets: Vec<ResourceHandle>
    }

    impl PassNode for MockPassNode {
        fn get_name(&self) -> &str {
            &self.name
        }

        fn get_inputs(&self) -> &[ResourceHandle] {
            &self.inputs
        }

        fn get_outputs(&self) -> &[ResourceHandle] {
            &self.outputs
        }

        fn get_rendertargets(&self) -> &[ResourceHandle] {
            &self.render_targets
        }
    }

    impl MockPassNode {
        pub fn new(
            name: String,
            inputs: Vec<ResourceHandle>,
            outputs: Vec<ResourceHandle>,
            render_targets: Vec<ResourceHandle>) -> MockPassNode {

            MockPassNode {
                name,
                inputs,
                outputs,
                render_targets
            }
        }
    }

    #[test]
    fn dummy_test() {
        println!("Running a test");
        assert_eq!(1, 1);
    }

    #[test]
    fn framegraph_sort() {
        let rm = MockResourceManager{};
        let mut frame_graph : FrameGraph<MockResourceManager, MockPassNode> = FrameGraph::new(rm);
        let resource_one = ResourceHandle::Transient(0);
        let resource_two = ResourceHandle::Transient(1);
        let resource_three = ResourceHandle::Transient(2);
        let n1 = MockPassNode::new(
            "One".to_string(),
            vec![],
        vec![],
        vec![resource_one]);
        let n2 = MockPassNode::new(
            "Two".to_string(),
            vec![resource_one],
            vec![],
            vec![resource_two]);
        let n3 = MockPassNode::new(
            "Three".to_string(),
            vec![resource_two, resource_three],
            vec![],
            vec![]);
        let n4 = MockPassNode::new(
            "Four".to_string(),
            vec![],
            vec![],
            vec![resource_three]);

        frame_graph.start();
        frame_graph.add_node(n1);
        frame_graph.add_node(n2);
        frame_graph.add_node(n3);
        frame_graph.add_node(n4);
        frame_graph.compile();
    }
}
