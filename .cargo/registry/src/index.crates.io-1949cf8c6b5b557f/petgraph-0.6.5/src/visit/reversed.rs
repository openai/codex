use crate::{Direction, Incoming};

use crate::visit::{
    Data, EdgeCount, EdgeIndexable, EdgeRef, GetAdjacencyMatrix, GraphBase, GraphProp, GraphRef,
    IntoEdgeReferences, IntoEdges, IntoEdgesDirected, IntoNeighbors, IntoNeighborsDirected,
    IntoNodeIdentifiers, IntoNodeReferences, NodeCompactIndexable, NodeCount, NodeIndexable,
    Visitable,
};

/// An edge-reversing graph adaptor.
///
/// All edges have the opposite direction with `Reversed`.
#[derive(Copy, Clone, Debug)]
pub struct Reversed<G>(pub G);

impl<G: GraphBase> GraphBase for Reversed<G> {
    type NodeId = G::NodeId;
    type EdgeId = G::EdgeId;
}

impl<G: GraphRef> GraphRef for Reversed<G> {}

Data! {delegate_impl [[G], G, Reversed<G>, access0]}

impl<G> IntoNeighbors for Reversed<G>
where
    G: IntoNeighborsDirected,
{
    type Neighbors = G::NeighborsDirected;
    fn neighbors(self, n: G::NodeId) -> G::NeighborsDirected {
        self.0.neighbors_directed(n, Incoming)
    }
}

impl<G> IntoNeighborsDirected for Reversed<G>
where
    G: IntoNeighborsDirected,
{
    type NeighborsDirected = G::NeighborsDirected;
    fn neighbors_directed(self, n: G::NodeId, d: Direction) -> G::NeighborsDirected {
        self.0.neighbors_directed(n, d.opposite())
    }
}

impl<G> IntoEdges for Reversed<G>
where
    G: IntoEdgesDirected,
{
    type Edges = ReversedEdges<G::EdgesDirected>;
    fn edges(self, a: Self::NodeId) -> Self::Edges {
        ReversedEdges {
            iter: self.0.edges_directed(a, Incoming),
        }
    }
}

impl<G> IntoEdgesDirected for Reversed<G>
where
    G: IntoEdgesDirected,
{
    type EdgesDirected = ReversedEdges<G::EdgesDirected>;
    fn edges_directed(self, a: Self::NodeId, dir: Direction) -> Self::Edges {
        ReversedEdges {
            iter: self.0.edges_directed(a, dir.opposite()),
        }
    }
}

impl<G: Visitable> Visitable for Reversed<G> {
    type Map = G::Map;
    fn visit_map(&self) -> G::Map {
        self.0.visit_map()
    }
    fn reset_map(&self, map: &mut Self::Map) {
        self.0.reset_map(map);
    }
}

/// A reversed edges iterator.
#[derive(Debug, Clone)]
pub struct ReversedEdges<I> {
    iter: I,
}

impl<I> Iterator for ReversedEdges<I>
where
    I: Iterator,
    I::Item: EdgeRef,
{
    type Item = ReversedEdgeReference<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(ReversedEdgeReference)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// A reversed edge reference
#[derive(Copy, Clone, Debug)]
pub struct ReversedEdgeReference<R>(R);

impl<R> ReversedEdgeReference<R> {
    /// Return the original, unreversed edge reference.
    pub fn as_unreversed(&self) -> &R {
        &self.0
    }

    /// Consume `self` and return the original, unreversed edge reference.
    pub fn into_unreversed(self) -> R {
        self.0
    }
}

/// An edge reference
impl<R> EdgeRef for ReversedEdgeReference<R>
where
    R: EdgeRef,
{
    type NodeId = R::NodeId;
    type EdgeId = R::EdgeId;
    type Weight = R::Weight;
    fn source(&self) -> Self::NodeId {
        self.0.target()
    }
    fn target(&self) -> Self::NodeId {
        self.0.source()
    }
    fn weight(&self) -> &Self::Weight {
        self.0.weight()
    }
    fn id(&self) -> Self::EdgeId {
        self.0.id()
    }
}

impl<G> IntoEdgeReferences for Reversed<G>
where
    G: IntoEdgeReferences,
{
    type EdgeRef = ReversedEdgeReference<G::EdgeRef>;
    type EdgeReferences = ReversedEdgeReferences<G::EdgeReferences>;
    fn edge_references(self) -> Self::EdgeReferences {
        ReversedEdgeReferences {
            iter: self.0.edge_references(),
        }
    }
}

/// A reversed edge references iterator.
#[derive(Debug, Clone)]
pub struct ReversedEdgeReferences<I> {
    iter: I,
}

impl<I> Iterator for ReversedEdgeReferences<I>
where
    I: Iterator,
    I::Item: EdgeRef,
{
    type Item = ReversedEdgeReference<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(ReversedEdgeReference)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

macro_rules! access0 {
    ($e:expr) => {
        $e.0
    };
}

NodeIndexable! {delegate_impl [[G], G, Reversed<G>, access0]}
NodeCompactIndexable! {delegate_impl [[G], G, Reversed<G>, access0]}
IntoNodeIdentifiers! {delegate_impl [[G], G, Reversed<G>, access0]}
IntoNodeReferences! {delegate_impl [[G], G, Reversed<G>, access0]}
GraphProp! {delegate_impl [[G], G, Reversed<G>, access0]}
NodeCount! {delegate_impl [[G], G, Reversed<G>, access0]}
EdgeCount! {delegate_impl [[G], G, Reversed<G>, access0]}
EdgeIndexable! {delegate_impl [[G], G, Reversed<G>, access0]}
GetAdjacencyMatrix! {delegate_impl [[G], G, Reversed<G>, access0]}
