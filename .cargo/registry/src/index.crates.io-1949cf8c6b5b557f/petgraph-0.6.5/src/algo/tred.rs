//! Compute the transitive reduction and closure of a directed acyclic graph
//!
//! ## Transitive reduction and closure
//! The *transitive closure* of a graph **G = (V, E)** is the graph **Gc = (V, Ec)**
//! such that **(i, j)** belongs to **Ec** if and only if there is a path connecting
//! **i** to **j** in **G**. The *transitive reduction* of **G** is the graph **Gr
//! = (V, Er)** such that **Er** is minimal wrt. inclusion in **E** and the transitive
//! closure of **Gr** is the same as that of **G**.
//! The transitive reduction is well-defined for acyclic graphs only.

use crate::adj::{List, UnweightedList};
use crate::graph::IndexType;
use crate::visit::{
    GraphBase, IntoNeighbors, IntoNeighborsDirected, NodeCompactIndexable, NodeCount,
};
use crate::Direction;
use fixedbitset::FixedBitSet;

/// Creates a representation of the same graph respecting topological order for use in `tred::dag_transitive_reduction_closure`.
///
/// `toposort` must be a topological order on the node indices of `g` (for example obtained
/// from [`toposort`]).
///
/// [`toposort`]: ../fn.toposort.html
///
/// Returns a pair of a graph `res` and the reciprocal of the topological sort `revmap`.
///
/// `res` is the same graph as `g` with the following differences:
/// * Node and edge weights are stripped,
/// * Node indices are replaced by the corresponding rank in `toposort`,
/// * Iterating on the neighbors of a node respects topological order.
///
/// `revmap` is handy to get back to map indices in `g` to indices in `res`.
/// ```
/// use petgraph::prelude::*;
/// use petgraph::graph::DefaultIx;
/// use petgraph::visit::IntoNeighbors;
/// use petgraph::algo::tred::dag_to_toposorted_adjacency_list;
///
/// let mut g = Graph::<&str, (), Directed, DefaultIx>::new();
/// let second = g.add_node("second child");
/// let top = g.add_node("top");
/// let first = g.add_node("first child");
/// g.extend_with_edges(&[(top, second), (top, first), (first, second)]);
///
/// let toposort = vec![top, first, second];
///
/// let (res, revmap) = dag_to_toposorted_adjacency_list(&g, &toposort);
///
/// // let's compute the children of top in topological order
/// let children: Vec<NodeIndex> = res
///     .neighbors(revmap[top.index()])
///     .map(|ix: NodeIndex| toposort[ix.index()])
///     .collect();
/// assert_eq!(children, vec![first, second])
/// ```
///
/// Runtime: **O(|V| + |E|)**.
///
/// Space complexity: **O(|V| + |E|)**.
pub fn dag_to_toposorted_adjacency_list<G, Ix: IndexType>(
    g: G,
    toposort: &[G::NodeId],
) -> (UnweightedList<Ix>, Vec<Ix>)
where
    G: GraphBase + IntoNeighborsDirected + NodeCompactIndexable + NodeCount,
    G::NodeId: IndexType,
{
    let mut res = List::with_capacity(g.node_count());
    // map from old node index to rank in toposort
    let mut revmap = vec![Ix::default(); g.node_bound()];
    for (ix, &old_ix) in toposort.iter().enumerate() {
        let ix = Ix::new(ix);
        revmap[old_ix.index()] = ix;
        let iter = g.neighbors_directed(old_ix, Direction::Incoming);
        let new_ix: Ix = res.add_node_with_capacity(iter.size_hint().0);
        debug_assert_eq!(new_ix.index(), ix.index());
        for old_pre in iter {
            let pre: Ix = revmap[old_pre.index()];
            res.add_edge(pre, ix, ());
        }
    }
    (res, revmap)
}

/// Computes the transitive reduction and closure of a DAG.
///
/// The algorithm implemented here comes from [On the calculation of
/// transitive reduction-closure of
/// orders](https://www.sciencedirect.com/science/article/pii/0012365X9390164O) by Habib, Morvan
/// and Rampon.
///
/// The input graph must be in a very specific format: an adjacency
/// list such that:
/// * Node indices are a toposort, and
/// * The neighbors of all nodes are stored in topological order.
/// To get such a representation, use the function [`dag_to_toposorted_adjacency_list`].
///
/// [`dag_to_toposorted_adjacency_list`]: ./fn.dag_to_toposorted_adjacency_list.html
///
/// The output is the pair of the transitive reduction and the transitive closure.
///
/// Runtime complexity: **O(|V| + \sum_{(x, y) \in Er} d(y))** where **d(y)**
/// denotes the outgoing degree of **y** in the transitive closure of **G**.
/// This is still **O(|V|³)** in the worst case like the naive algorithm but
/// should perform better for some classes of graphs.
///
/// Space complexity: **O(|E|)**.
pub fn dag_transitive_reduction_closure<E, Ix: IndexType>(
    g: &List<E, Ix>,
) -> (UnweightedList<Ix>, UnweightedList<Ix>) {
    let mut tred = List::with_capacity(g.node_count());
    let mut tclos = List::with_capacity(g.node_count());
    let mut mark = FixedBitSet::with_capacity(g.node_count());
    for i in g.node_indices() {
        tred.add_node();
        tclos.add_node_with_capacity(g.neighbors(i).len());
    }
    // the algorithm relies on this iterator being toposorted
    for i in g.node_indices().rev() {
        // the algorighm relies on this iterator being toposorted
        for x in g.neighbors(i) {
            if !mark[x.index()] {
                tred.add_edge(i, x, ());
                tclos.add_edge(i, x, ());
                for e in tclos.edge_indices_from(x) {
                    let y = tclos.edge_endpoints(e).unwrap().1;
                    if !mark[y.index()] {
                        mark.insert(y.index());
                        tclos.add_edge(i, y, ());
                    }
                }
            }
        }
        for y in tclos.neighbors(i) {
            mark.set(y.index(), false);
        }
    }
    (tred, tclos)
}

#[cfg(test)]
#[test]
fn test_easy_tred() {
    let mut input = List::new();
    let a: u8 = input.add_node();
    let b = input.add_node();
    let c = input.add_node();
    input.add_edge(a, b, ());
    input.add_edge(a, c, ());
    input.add_edge(b, c, ());
    let (tred, tclos) = dag_transitive_reduction_closure(&input);
    assert_eq!(tred.node_count(), 3);
    assert_eq!(tclos.node_count(), 3);
    assert!(tred.find_edge(a, b).is_some());
    assert!(tred.find_edge(b, c).is_some());
    assert!(tred.find_edge(a, c).is_none());
    assert!(tclos.find_edge(a, b).is_some());
    assert!(tclos.find_edge(b, c).is_some());
    assert!(tclos.find_edge(a, c).is_some());
}
