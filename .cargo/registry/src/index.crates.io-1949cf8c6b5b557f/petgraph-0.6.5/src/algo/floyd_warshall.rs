use std::collections::HashMap;

use std::hash::Hash;

use crate::algo::{BoundedMeasure, NegativeCycle};
use crate::visit::{
    EdgeRef, GraphProp, IntoEdgeReferences, IntoNodeIdentifiers, NodeCompactIndexable,
};

#[allow(clippy::type_complexity, clippy::needless_range_loop)]
/// \[Generic\] [Floyd–Warshall algorithm](https://en.wikipedia.org/wiki/Floyd%E2%80%93Warshall_algorithm) is an algorithm for all pairs shortest path problem
///
/// Compute shortest paths in a weighted graph with positive or negative edge weights (but with no negative cycles)
///
/// # Arguments
/// * `graph`: graph with no negative cycle
/// * `edge_cost`: closure that returns cost of a particular edge
///
/// # Returns
/// * `Ok`: (if graph contains no negative cycle) a hashmap containing all pairs shortest paths
/// * `Err`: if graph contains negative cycle.
///
/// # Examples
/// ```rust
/// use petgraph::{prelude::*, Graph, Directed};
/// use petgraph::algo::floyd_warshall;
/// use std::collections::HashMap;
///
/// let mut graph: Graph<(), (), Directed> = Graph::new();
/// let a = graph.add_node(());
/// let b = graph.add_node(());
/// let c = graph.add_node(());
/// let d = graph.add_node(());
///
/// graph.extend_with_edges(&[
///    (a, b),
///    (a, c),
///    (a, d),
///    (b, c),
///    (b, d),
///    (c, d)
/// ]);
///
/// let weight_map: HashMap<(NodeIndex, NodeIndex), i32> = [
///    ((a, a), 0), ((a, b), 1), ((a, c), 4), ((a, d), 10),
///    ((b, b), 0), ((b, c), 2), ((b, d), 2),
///    ((c, c), 0), ((c, d), 2)
/// ].iter().cloned().collect();
/// //     ----- b --------
/// //    |      ^         | 2
/// //    |    1 |    4    v
/// //  2 |      a ------> c
/// //    |   10 |         | 2
/// //    |      v         v
/// //     --->  d <-------
///
/// let inf = std::i32::MAX;
/// let expected_res: HashMap<(NodeIndex, NodeIndex), i32> = [
///    ((a, a), 0), ((a, b), 1), ((a, c), 3), ((a, d), 3),
///    ((b, a), inf), ((b, b), 0), ((b, c), 2), ((b, d), 2),
///    ((c, a), inf), ((c, b), inf), ((c, c), 0), ((c, d), 2),
///    ((d, a), inf), ((d, b), inf), ((d, c), inf), ((d, d), 0),
/// ].iter().cloned().collect();
///
///
/// let res = floyd_warshall(&graph, |edge| {
///     if let Some(weight) = weight_map.get(&(edge.source(), edge.target())) {
///         *weight
///     } else {
///         inf
///     }
/// }).unwrap();
///
/// let nodes = [a, b, c, d];
/// for node1 in &nodes {
///     for node2 in &nodes {
///         assert_eq!(res.get(&(*node1, *node2)).unwrap(), expected_res.get(&(*node1, *node2)).unwrap());
///     }
/// }
/// ```
pub fn floyd_warshall<G, F, K>(
    graph: G,
    mut edge_cost: F,
) -> Result<HashMap<(G::NodeId, G::NodeId), K>, NegativeCycle>
where
    G: NodeCompactIndexable + IntoEdgeReferences + IntoNodeIdentifiers + GraphProp,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: BoundedMeasure + Copy,
{
    let num_of_nodes = graph.node_count();

    // |V|x|V| matrix
    let mut dist = vec![vec![K::max(); num_of_nodes]; num_of_nodes];

    // init distances of paths with no intermediate nodes
    for edge in graph.edge_references() {
        dist[graph.to_index(edge.source())][graph.to_index(edge.target())] = edge_cost(edge);
        if !graph.is_directed() {
            dist[graph.to_index(edge.target())][graph.to_index(edge.source())] = edge_cost(edge);
        }
    }

    // distance of each node to itself is 0(default value)
    for node in graph.node_identifiers() {
        dist[graph.to_index(node)][graph.to_index(node)] = K::default();
    }

    for k in 0..num_of_nodes {
        for i in 0..num_of_nodes {
            for j in 0..num_of_nodes {
                let (result, overflow) = dist[i][k].overflowing_add(dist[k][j]);
                if !overflow && dist[i][j] > result {
                    dist[i][j] = result;
                }
            }
        }
    }

    // value less than 0(default value) indicates a negative cycle
    for i in 0..num_of_nodes {
        if dist[i][i] < K::default() {
            return Err(NegativeCycle(()));
        }
    }

    let mut distance_map: HashMap<(G::NodeId, G::NodeId), K> =
        HashMap::with_capacity(num_of_nodes * num_of_nodes);

    for i in 0..num_of_nodes {
        for j in 0..num_of_nodes {
            distance_map.insert((graph.from_index(i), graph.from_index(j)), dist[i][j]);
        }
    }

    Ok(distance_map)
}
