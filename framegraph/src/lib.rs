use ash::vk;

pub mod frame_graph;
pub mod resource;
pub mod pipeline;
pub mod shader;
mod i_pass_node;

#[cfg(test)]
mod tests
{
    use std::process::Command;
    use ash::vk;
    use context::i_render_context::{RenderContext, CommandBuffer};
    use crate::resource::i_resource_manager::ResourceManager;
    use crate::resource::resource_manager::{ResourceHandle, ResolvedResourceMap, ResolvedResource, ResourceType};
    use crate::i_pass_node::PassNode;
    use crate::frame_graph::FrameGraph;

    struct MockRenderContext {
        nodes_executed: u32
    }

    impl RenderContext for MockRenderContext {

    }

    struct MockCommandBuffer {

    }

    impl CommandBuffer for MockCommandBuffer {

    }

    type FillCallback<RCType: RenderContext, CBType: CommandBuffer> = dyn (
    Fn(
        &mut RCType,
        &CBType,
        &ResolvedResourceMap,
        &ResolvedResourceMap,
        u32
    )
    );


    struct MockResourceManager {

    }

    impl ResourceManager for MockResourceManager {
        fn resolve_resource(&mut self, handle: &ResourceHandle) -> ResolvedResource {
            ResolvedResource {
                handle: *handle,
                resource: ResourceType::Buffer(vk::Buffer::null())
            }
        }
    }

    struct MockPassNode<RCType: RenderContext, CBType: CommandBuffer> {
        name: String,
        inputs: Vec<ResourceHandle>,
        outputs: Vec<ResourceHandle>,
        render_targets: Vec<ResourceHandle>,
        intended_order: u32,
        callback: Box<FillCallback<RCType, CBType>>
    }

    impl<RCType: RenderContext, CBType: CommandBuffer> PassNode<RCType, CBType> for MockPassNode<RCType, CBType> {
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

        // fn execute<RCType: RenderContext, CBType: CommandBuffer> (
        fn execute(
            &self,
            render_context: &mut RCType,
            command_buffer: &CBType,
            resolved_inputs: &ResolvedResourceMap,
            resolved_outputs: &ResolvedResourceMap) {

            (self.callback)(
                render_context,
                command_buffer,
                resolved_inputs,
                resolved_outputs,
                self.intended_order);
        }
    }

    impl<RCType: RenderContext, CBType: CommandBuffer> MockPassNode<RCType, CBType> {
        pub fn new(
            name: String,
            inputs: Vec<ResourceHandle>,
            outputs: Vec<ResourceHandle>,
            render_targets: Vec<ResourceHandle>,
            intended_order: u32,
            callback: Box<FillCallback<RCType, CBType>>) -> MockPassNode<RCType, CBType> {

            MockPassNode {
                name,
                inputs,
                outputs,
                render_targets,
                intended_order,
                callback
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
        let mut render_context = MockRenderContext{
            nodes_executed: 0
        };
        let command_buffer = MockCommandBuffer{};
        let rm = MockResourceManager{};
        let mut frame_graph : FrameGraph<MockRenderContext, MockCommandBuffer, MockResourceManager, MockPassNode<MockRenderContext, MockCommandBuffer>> = FrameGraph::new(rm);
        let resource_one = ResourceHandle::Transient(0);
        let resource_two = ResourceHandle::Transient(1);
        let resource_three = ResourceHandle::Transient(2);
        let unbound_resource = ResourceHandle::Transient(3);
        let mock_callback = |render_ctx: &mut MockRenderContext,
            command_buffer: &MockCommandBuffer,
            inputs: &ResolvedResourceMap,
            outputs: &ResolvedResourceMap,
            intended_order: u32| {
            assert_eq!(
                render_ctx.nodes_executed,
                intended_order,
                "Nodes did not execute in the expected order: \n\tExpected {}, was executed at {}",
                    intended_order,
                    render_ctx.nodes_executed);
            render_ctx.nodes_executed += 1;
        };
        let mock_callback_fail = |render_ctx: &mut MockRenderContext,
            command_buffer: &MockCommandBuffer,
            inputs: &ResolvedResourceMap,
            outputs: &ResolvedResourceMap,
            intended_order: u32| {
            assert!(false, "This node should have been rejected");
        };
        let n1 = MockPassNode::new(
            "One".to_string(),
            vec![],
            vec![],
            vec![resource_one],
            1,
            Box::new(mock_callback));
        let n2 = MockPassNode::new(
            "Two".to_string(),
            vec![resource_one],
            vec![],
            vec![resource_two],
            2,
            Box::new(mock_callback));
        let n3 = MockPassNode::new(
            "Three".to_string(),
            vec![resource_two, resource_three],
            vec![],
            vec![],
            3,
            Box::new(mock_callback));
        let n4 = MockPassNode::new(
            "Four".to_string(),
            vec![],
            vec![],
            vec![resource_three],
            0,
            Box::new(mock_callback));
        let n5 = MockPassNode::new(
            "Five".to_string(),
            vec![unbound_resource],
            vec![],
            vec![],
            5,
            Box::new(mock_callback_fail));

        frame_graph.start();
        frame_graph.add_node(n1);
        frame_graph.add_node(n2);
        frame_graph.add_node(n3);
        frame_graph.add_node(n4);
        frame_graph.add_node(n5);
        frame_graph.compile();
        frame_graph.end(&mut render_context, &command_buffer);
    }
}
