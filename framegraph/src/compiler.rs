use std::sync::RwLock;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::Dfs;
use multimap::MultiMap;
use log::trace as log_trace;
use crate::pass_type::PassType;

#[tracing::instrument(skip(nodes))]
pub fn compile(nodes: &mut StableDiGraph<RwLock<PassType>, u32>, root_index: NodeIndex) -> Vec<NodeIndex> {
    // create input/output maps to detect graph edges
    let mut input_map = MultiMap::new();
    let mut output_map = MultiMap::new();
    for node_index in nodes.node_indices() {
        let node = &nodes[node_index].read().unwrap();
        for read in node.get_reads() {
            input_map.insert(read, node_index);
        }
        for write in node.get_writes() {
            output_map.insert(write, node_index);
        }
    }

    // iterate over input map. For each input, find matching outputs and then
    // generate a graph edge for each pairing
    for (input, node_index) in input_map.iter() {
        let find_outputs = output_map.get_vec(input);
        if let Some(matched_outputs) = find_outputs {
            // input/output match defines a graph edge
            for matched_output in matched_outputs {
                // use update_edge instead of add_edge to avoid duplicates
                // if matched_output.index() != node_index.index() {
                if matched_output != node_index {
                    nodes.update_edge(
                        *node_index,
                        *matched_output,
                        0);
                }
            }
        }
    }

    // Use DFS to find all accessible nodes from the root node
    {
        let mut retained_nodes: Vec<bool> = Vec::new();
        retained_nodes.resize(nodes.node_count(), false);

        //let mut dfs = Dfs::new(&nodes, root_index);
        let mut dfs = Dfs::new(&*nodes, root_index);
        while let Some(node_id) = dfs.next(&*nodes) {
            retained_nodes[node_id.index()] = true;
        }

        nodes.retain_nodes(|_graph, node_index| {
            retained_nodes[node_index.index()]
        });
    }

    // unresolved and unused passes have been removed from the graph,
    // so now we can use a topological sort to generate an execution order
    let sorted_nodes: Vec<NodeIndex>;
    {
        let sort_result = petgraph::algo::toposort(&*nodes, None);
        match sort_result {
            Ok(mut sorted_list) => {
                // DFS requires we order nodes as input -> output, but for sorting we want output -> input
                sorted_list.reverse();
                for i in &sorted_list {
                    log_trace!(target: "framegraph", "Sorted node: {:?}", nodes.node_weight(*i).unwrap().read().unwrap().get_name())
                }
                sorted_nodes = sorted_list;
            },
            Err(cycle_error) => {
                panic!("A cycle was detected in the framegraph: {:?}", cycle_error);
            }
        }
    }

    sorted_nodes
}
