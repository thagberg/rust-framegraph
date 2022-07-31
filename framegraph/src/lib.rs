pub mod frame_graph;
pub mod pass_node;
pub mod resource;
pub mod pipeline;
pub mod shader;

#[cfg(test)]
mod tests
{
    #[test]
    fn dummy_test() {
        println!("Running a test");
        assert_eq!(1, 1);
    }
}
