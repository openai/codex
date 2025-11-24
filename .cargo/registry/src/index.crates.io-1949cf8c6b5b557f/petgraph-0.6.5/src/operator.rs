//! Operators for creating new graphs from existings ones.
use super::graph::{Graph, IndexType};
use super::EdgeType;
use crate::visit::IntoNodeReferences;

/// \[Generic\] complement of the graph
///
/// Computes the graph complement of the input Graph and stores it
/// in the provided empty output Graph.
///
/// The function does not create self-loops.
///
/// Computes in **O(|V|^2*log(|V|))** time (average).
///
/// Returns the complement.
///
/// # Example
/// ```rust
/// use petgraph::Graph;
/// use petgraph::operator::complement;
/// use petgraph::prelude::*;
///
/// let mut graph: Graph<(),(),Directed> = Graph::new();
/// let a = graph.add_node(()); // node with no weight
/// let b = graph.add_node(());
/// let c = graph.add_node(());
/// let d = graph.add_node(());
///
/// graph.extend_with_edges(&[
///     (a, b),
///     (b, c),
///     (c, d),
/// ]);
/// // a ----> b ----> c ----> d
///
/// let mut output: Graph<(), (), Directed> = Graph::new();
///
/// complement(&graph, &mut output, ());
///
/// let mut expected_res: Graph<(), (), Directed> = Graph::new();
/// let a = expected_res.add_node(());
/// let b = expected_res.add_node(());
/// let c = expected_res.add_node(());
/// let d = expected_res.add_node(());
/// expected_res.extend_with_edges(&[
///     (a, c),
///     (a, d),
///     (b, a),
///     (b, d),
///     (c, a),
///     (c, b),
///     (d, a),
///     (d, b),
///     (d, c),
/// ]);
///
/// for x in graph.node_indices() {
///     for y in graph.node_indices() {
///         assert_eq!(output.contains_edge(x, y), expected_res.contains_edge(x, y));
///     }
/// }
/// ```
pub fn complement<N, E, Ty, Ix>(
    input: &Graph<N, E, Ty, Ix>,
    output: &mut Graph<N, E, Ty, Ix>,
    weight: E,
) where
    Ty: EdgeType,
    Ix: IndexType,
    E: Clone,
    N: Clone,
{
    for (_node, weight) in input.node_references() {
        output.add_node(weight.clone());
    }
    for x in input.node_indices() {
        for y in input.node_indices() {
            if x != y && !input.contains_edge(x, y) {
                output.add_edge(x, y, weight.clone());
            }
        }
    }
}
