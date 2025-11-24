use std::collections::{BinaryHeap, HashMap};

use std::hash::Hash;

use crate::algo::Measure;
use crate::scored::MinScored;
use crate::visit::{EdgeRef, IntoEdges, NodeCount, NodeIndexable, Visitable};

/// \[Generic\] k'th shortest path algorithm.
///
/// Compute the length of the k'th shortest path from `start` to every reachable
/// node.
///
/// The graph should be `Visitable` and implement `IntoEdges`. The function
/// `edge_cost` should return the cost for a particular edge, which is used
/// to compute path costs. Edge costs must be non-negative.
///
/// If `goal` is not `None`, then the algorithm terminates once the `goal` node's
/// cost is calculated.
///
/// Computes in **O(k * (|E| + |V|*log(|V|)))** time (average).
///
/// Returns a `HashMap` that maps `NodeId` to path cost.
/// # Example
/// ```rust
/// use petgraph::Graph;
/// use petgraph::algo::k_shortest_path;
/// use petgraph::prelude::*;
/// use std::collections::HashMap;
///
/// let mut graph : Graph<(),(),Directed>= Graph::new();
/// let a = graph.add_node(()); // node with no weight
/// let b = graph.add_node(());
/// let c = graph.add_node(());
/// let d = graph.add_node(());
/// let e = graph.add_node(());
/// let f = graph.add_node(());
/// let g = graph.add_node(());
/// let h = graph.add_node(());
/// // z will be in another connected component
/// let z = graph.add_node(());
///
/// graph.extend_with_edges(&[
///     (a, b),
///     (b, c),
///     (c, d),
///     (d, a),
///     (e, f),
///     (b, e),
///     (f, g),
///     (g, h),
///     (h, e)
/// ]);
/// // a ----> b ----> e ----> f
/// // ^       |       ^       |
/// // |       v       |       v
/// // d <---- c       h <---- g
///
/// let expected_res: HashMap<NodeIndex, usize> = [
///      (a, 7),
///      (b, 4),
///      (c, 5),
///      (d, 6),
///      (e, 5),
///      (f, 6),
///      (g, 7),
///      (h, 8)
///     ].iter().cloned().collect();
/// let res = k_shortest_path(&graph,b,None,2, |_| 1);
/// assert_eq!(res, expected_res);
/// // z is not inside res because there is not path from b to z.
/// ```
pub fn k_shortest_path<G, F, K>(
    graph: G,
    start: G::NodeId,
    goal: Option<G::NodeId>,
    k: usize,
    mut edge_cost: F,
) -> HashMap<G::NodeId, K>
where
    G: IntoEdges + Visitable + NodeCount + NodeIndexable,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: Measure + Copy,
{
    let mut counter: Vec<usize> = vec![0; graph.node_count()];
    let mut scores = HashMap::new();
    let mut visit_next = BinaryHeap::new();
    let zero_score = K::default();

    visit_next.push(MinScored(zero_score, start));

    while let Some(MinScored(node_score, node)) = visit_next.pop() {
        counter[graph.to_index(node)] += 1;
        let current_counter = counter[graph.to_index(node)];

        if current_counter > k {
            continue;
        }

        if current_counter == k {
            scores.insert(node, node_score);
        }

        //Already reached goal k times
        if goal.as_ref() == Some(&node) && current_counter == k {
            break;
        }

        for edge in graph.edges(node) {
            visit_next.push(MinScored(node_score + edge_cost(edge), edge.target()));
        }
    }
    scores
}
