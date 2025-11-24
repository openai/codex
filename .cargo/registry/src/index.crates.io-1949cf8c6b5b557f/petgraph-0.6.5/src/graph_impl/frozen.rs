use std::ops::{Deref, Index, IndexMut};

use super::Frozen;
use crate::data::{DataMap, DataMapMut};
use crate::graph::Graph;
use crate::graph::{GraphIndex, IndexType};
use crate::visit::{
    Data, EdgeCount, EdgeIndexable, GetAdjacencyMatrix, GraphBase, GraphProp, IntoEdges,
    IntoEdgesDirected, IntoNeighborsDirected, IntoNodeIdentifiers, NodeCompactIndexable, NodeCount,
    NodeIndexable,
};
use crate::visit::{IntoEdgeReferences, IntoNeighbors, IntoNodeReferences, Visitable};
use crate::{Direction, EdgeType};

impl<'a, G> Frozen<'a, G> {
    /// Create a new `Frozen` from a mutable reference to a graph.
    pub fn new(gr: &'a mut G) -> Self {
        Frozen(gr)
    }
}

/// Deref allows transparent access to all shared reference (read-only)
/// functionality in the underlying graph.
impl<'a, G> Deref for Frozen<'a, G> {
    type Target = G;
    fn deref(&self) -> &G {
        self.0
    }
}

impl<'a, G, I> Index<I> for Frozen<'a, G>
where
    G: Index<I>,
{
    type Output = G::Output;
    fn index(&self, i: I) -> &G::Output {
        self.0.index(i)
    }
}

impl<'a, G, I> IndexMut<I> for Frozen<'a, G>
where
    G: IndexMut<I>,
{
    fn index_mut(&mut self, i: I) -> &mut G::Output {
        self.0.index_mut(i)
    }
}

impl<'a, N, E, Ty, Ix> Frozen<'a, Graph<N, E, Ty, Ix>>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    #[allow(clippy::type_complexity)]
    /// Index the `Graph` by two indices, any combination of
    /// node or edge indices is fine.
    ///
    /// **Panics** if the indices are equal or if they are out of bounds.
    pub fn index_twice_mut<T, U>(
        &mut self,
        i: T,
        j: U,
    ) -> (
        &mut <Graph<N, E, Ty, Ix> as Index<T>>::Output,
        &mut <Graph<N, E, Ty, Ix> as Index<U>>::Output,
    )
    where
        Graph<N, E, Ty, Ix>: IndexMut<T> + IndexMut<U>,
        T: GraphIndex,
        U: GraphIndex,
    {
        self.0.index_twice_mut(i, j)
    }
}

macro_rules! access0 {
    ($e:expr) => {
        $e.0
    };
}

impl<'a, G> GraphBase for Frozen<'a, G>
where
    G: GraphBase,
{
    type NodeId = G::NodeId;
    type EdgeId = G::EdgeId;
}

Data! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
DataMap! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
DataMapMut! {delegate_impl [['a, G], G, Frozen<'a, G>, access0]}
GetAdjacencyMatrix! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
IntoEdgeReferences! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
IntoEdges! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
IntoEdgesDirected! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
IntoNeighbors! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
IntoNeighborsDirected! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
IntoNodeIdentifiers! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
IntoNodeReferences! {delegate_impl [['a, 'b, G], G, &'b Frozen<'a, G>, deref_twice]}
NodeCompactIndexable! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
NodeCount! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
NodeIndexable! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
EdgeCount! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
EdgeIndexable! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
GraphProp! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
Visitable! {delegate_impl [['a, G], G, Frozen<'a, G>, deref_twice]}
