use std::{
    hash::Hash,
    iter::{from_fn, FromIterator},
};

use indexmap::IndexSet;

use crate::{
    visit::{IntoNeighborsDirected, NodeCount},
    Direction::Outgoing,
};

/// Returns an iterator that produces all simple paths from `from` node to `to`, which contains at least `min_intermediate_nodes` nodes
/// and at most `max_intermediate_nodes`, if given, or limited by the graph's order otherwise. The simple path is a path without repetitions.
///
/// This algorithm is adapted from <https://networkx.github.io/documentation/stable/reference/algorithms/generated/networkx.algorithms.simple_paths.all_simple_paths.html>.
///
/// # Example
/// ```
/// use petgraph::{algo, prelude::*};
///
/// let mut graph = DiGraph::<&str, i32>::new();
///
/// let a = graph.add_node("a");
/// let b = graph.add_node("b");
/// let c = graph.add_node("c");
/// let d = graph.add_node("d");
///
/// graph.extend_with_edges(&[(a, b, 1), (b, c, 1), (c, d, 1), (a, b, 1), (b, d, 1)]);
///
/// let ways = algo::all_simple_paths::<Vec<_>, _>(&graph, a, d, 0, None)
///   .collect::<Vec<_>>();
///
/// assert_eq!(4, ways.len());
/// ```
pub fn all_simple_paths<TargetColl, G>(
    graph: G,
    from: G::NodeId,
    to: G::NodeId,
    min_intermediate_nodes: usize,
    max_intermediate_nodes: Option<usize>,
) -> impl Iterator<Item = TargetColl>
where
    G: NodeCount,
    G: IntoNeighborsDirected,
    G::NodeId: Eq + Hash,
    TargetColl: FromIterator<G::NodeId>,
{
    // how many nodes are allowed in simple path up to target node
    // it is min/max allowed path length minus one, because it is more appropriate when implementing lookahead
    // than constantly add 1 to length of current path
    let max_length = if let Some(l) = max_intermediate_nodes {
        l + 1
    } else {
        graph.node_count() - 1
    };

    let min_length = min_intermediate_nodes + 1;

    // list of visited nodes
    let mut visited: IndexSet<G::NodeId> = IndexSet::from_iter(Some(from));
    // list of childs of currently exploring path nodes,
    // last elem is list of childs of last visited node
    let mut stack = vec![graph.neighbors_directed(from, Outgoing)];

    from_fn(move || {
        while let Some(children) = stack.last_mut() {
            if let Some(child) = children.next() {
                if visited.len() < max_length {
                    if child == to {
                        if visited.len() >= min_length {
                            let path = visited
                                .iter()
                                .cloned()
                                .chain(Some(to))
                                .collect::<TargetColl>();
                            return Some(path);
                        }
                    } else if !visited.contains(&child) {
                        visited.insert(child);
                        stack.push(graph.neighbors_directed(child, Outgoing));
                    }
                } else {
                    if (child == to || children.any(|v| v == to)) && visited.len() >= min_length {
                        let path = visited
                            .iter()
                            .cloned()
                            .chain(Some(to))
                            .collect::<TargetColl>();
                        return Some(path);
                    }
                    stack.pop();
                    visited.pop();
                }
            } else {
                stack.pop();
                visited.pop();
            }
        }
        None
    })
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, iter::FromIterator};

    use itertools::assert_equal;

    use crate::{dot::Dot, prelude::DiGraph};

    use super::all_simple_paths;

    #[test]
    fn test_all_simple_paths() {
        let graph = DiGraph::<i32, i32, _>::from_edges(&[
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 2),
            (1, 3),
            (2, 3),
            (2, 4),
            (3, 2),
            (3, 4),
            (4, 2),
            (4, 5),
            (5, 2),
            (5, 3),
        ]);

        let expexted_simple_paths_0_to_5 = vec![
            vec![0usize, 1, 2, 3, 4, 5],
            vec![0, 1, 2, 4, 5],
            vec![0, 1, 3, 2, 4, 5],
            vec![0, 1, 3, 4, 5],
            vec![0, 2, 3, 4, 5],
            vec![0, 2, 4, 5],
            vec![0, 3, 2, 4, 5],
            vec![0, 3, 4, 5],
        ];

        println!("{}", Dot::new(&graph));
        let actual_simple_paths_0_to_5: HashSet<Vec<_>> =
            all_simple_paths(&graph, 0u32.into(), 5u32.into(), 0, None)
                .map(|v: Vec<_>| v.into_iter().map(|i| i.index()).collect())
                .collect();
        assert_eq!(actual_simple_paths_0_to_5.len(), 8);
        assert_eq!(
            HashSet::from_iter(expexted_simple_paths_0_to_5),
            actual_simple_paths_0_to_5
        );
    }

    #[test]
    fn test_one_simple_path() {
        let graph = DiGraph::<i32, i32, _>::from_edges(&[(0, 1), (2, 1)]);

        let expexted_simple_paths_0_to_1 = &[vec![0usize, 1]];
        println!("{}", Dot::new(&graph));
        let actual_simple_paths_0_to_1: Vec<Vec<_>> =
            all_simple_paths(&graph, 0u32.into(), 1u32.into(), 0, None)
                .map(|v: Vec<_>| v.into_iter().map(|i| i.index()).collect())
                .collect();

        assert_eq!(actual_simple_paths_0_to_1.len(), 1);
        assert_equal(expexted_simple_paths_0_to_1, &actual_simple_paths_0_to_1);
    }

    #[test]
    fn test_no_simple_paths() {
        let graph = DiGraph::<i32, i32, _>::from_edges(&[(0, 1), (2, 1)]);

        println!("{}", Dot::new(&graph));
        let actual_simple_paths_0_to_2: Vec<Vec<_>> =
            all_simple_paths(&graph, 0u32.into(), 2u32.into(), 0, None)
                .map(|v: Vec<_>| v.into_iter().map(|i| i.index()).collect())
                .collect();

        assert_eq!(actual_simple_paths_0_to_2.len(), 0);
    }
}
