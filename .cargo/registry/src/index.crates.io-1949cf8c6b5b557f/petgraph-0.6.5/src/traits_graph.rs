use fixedbitset::FixedBitSet;

use super::EdgeType;

use super::graph::{Graph, IndexType, NodeIndex};
#[cfg(feature = "stable_graph")]
use crate::stable_graph::StableGraph;
use crate::visit::EdgeRef;
#[cfg(feature = "stable_graph")]
use crate::visit::{IntoEdgeReferences, NodeIndexable};

use super::visit::GetAdjacencyMatrix;

/// The adjacency matrix for **Graph** is a bitmap that's computed by
/// `.adjacency_matrix()`.
impl<N, E, Ty, Ix> GetAdjacencyMatrix for Graph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    type AdjMatrix = FixedBitSet;

    fn adjacency_matrix(&self) -> FixedBitSet {
        let n = self.node_count();
        let mut matrix = FixedBitSet::with_capacity(n * n);
        for edge in self.edge_references() {
            let i = edge.source().index() * n + edge.target().index();
            matrix.put(i);
            if !self.is_directed() {
                let j = edge.source().index() + n * edge.target().index();
                matrix.put(j);
            }
        }
        matrix
    }

    fn is_adjacent(&self, matrix: &FixedBitSet, a: NodeIndex<Ix>, b: NodeIndex<Ix>) -> bool {
        let n = self.node_count();
        let index = n * a.index() + b.index();
        matrix.contains(index)
    }
}

#[cfg(feature = "stable_graph")]
/// The adjacency matrix for **Graph** is a bitmap that's computed by
/// `.adjacency_matrix()`.
impl<N, E, Ty, Ix> GetAdjacencyMatrix for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    type AdjMatrix = FixedBitSet;

    fn adjacency_matrix(&self) -> FixedBitSet {
        let n = self.node_bound();
        let mut matrix = FixedBitSet::with_capacity(n * n);
        for edge in self.edge_references() {
            let i = edge.source().index() * n + edge.target().index();
            matrix.put(i);
            if !self.is_directed() {
                let j = edge.source().index() + n * edge.target().index();
                matrix.put(j);
            }
        }
        matrix
    }

    fn is_adjacent(&self, matrix: &FixedBitSet, a: NodeIndex<Ix>, b: NodeIndex<Ix>) -> bool {
        let n = self.node_count();
        let index = n * a.index() + b.index();
        matrix.contains(index)
    }
}
