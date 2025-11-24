//! Graph traits and graph traversals.
//!
//! ### The `Into-` Traits
//!
//! Graph traits like [`IntoNeighbors`][in] create iterators and use the same
//! pattern that `IntoIterator` does: the trait takes a reference to a graph,
//! and produces an iterator. These traits are quite composable, but with the
//! limitation that they only use shared references to graphs.
//!
//! ### Graph Traversal
//!
//! [`Dfs`](struct.Dfs.html), [`Bfs`][bfs], [`DfsPostOrder`][dfspo] and
//! [`Topo`][topo]  are basic visitors and they use “walker” methods: the
//! visitors don't hold the graph as borrowed during traversal, only for the
//! `.next()` call on the walker. They can be converted to iterators
//! through the [`Walker`][w] trait.
//!
//! There is also the callback based traversal [`depth_first_search`][dfs].
//!
//! [bfs]: struct.Bfs.html
//! [dfspo]: struct.DfsPostOrder.html
//! [topo]: struct.Topo.html
//! [dfs]: fn.depth_first_search.html
//! [w]: trait.Walker.html
//!
//! ### Other Graph Traits
//!
//! The traits are rather loosely coupled at the moment (which is intentional,
//! but will develop a bit), and there are traits missing that could be added.
//!
//! Not much is needed to be able to use the visitors on a graph. A graph
//! needs to define [`GraphBase`][gb], [`IntoNeighbors`][in] and
//! [`Visitable`][vis] as a minimum.
//!
//! [gb]: trait.GraphBase.html
//! [in]: trait.IntoNeighbors.html
//! [vis]: trait.Visitable.html
//!
//! ### Graph Trait Implementations
//!
//! The following table lists the traits that are implemented for each graph type:
//!
//! |                       | Graph | StableGraph | GraphMap | MatrixGraph | Csr   | List  |
//! | --------------------- | :---: | :---------: | :------: | :---------: | :---: | :---: |
//! | GraphBase             | x     |  x          |    x     | x           | x     |  x    |
//! | GraphProp             | x     |  x          |    x     | x           | x     |  x    |
//! | NodeCount             | x     |  x          |    x     | x           | x     |  x    |
//! | NodeIndexable         | x     |  x          |    x     | x           | x     |  x    |
//! | NodeCompactIndexable  | x     |             |    x     |             | x     |  x    |
//! | EdgeCount             | x     |  x          |    x     | x           | x     |  x    |
//! | EdgeIndexable         | x     |  x          |    x     |             |       |       |
//! | Data                  | x     |  x          |    x     | x           | x     |  x    |
//! | IntoNodeIdentifiers   | x     |  x          |    x     | x           | x     |  x    |
//! | IntoNodeReferences    | x     |  x          |    x     | x           | x     |  x    |
//! | IntoEdgeReferences    | x     |  x          |    x     | x           | x     |  x    |
//! | IntoNeighbors         | x     |  x          |    x     | x           | x     |  x    |
//! | IntoNeighborsDirected | x     |  x          |    x     | x           |       |       |
//! | IntoEdges             | x     |  x          |    x     | x           | x     |  x    |
//! | IntoEdgesDirected     | x     |  x          |    x     | x           |       |       |
//! | Visitable             | x     |  x          |    x     | x           | x     |  x    |
//! | GetAdjacencyMatrix    | x     |  x          |    x     | x           | x     |  x    |

// filter, reversed have their `mod` lines at the end,
// so that they can use the trait template macros
pub use self::filter::*;
pub use self::reversed::*;

#[macro_use]
mod macros;

mod dfsvisit;
mod traversal;
pub use self::dfsvisit::*;
pub use self::traversal::*;

use fixedbitset::FixedBitSet;
use std::collections::HashSet;
use std::hash::{BuildHasher, Hash};

use super::EdgeType;
use crate::prelude::Direction;

use crate::graph::IndexType;

trait_template! {
/// Base graph trait: defines the associated node identifier and
/// edge identifier types.
pub trait GraphBase {
    // FIXME: We can drop this escape/nodelegate stuff in Rust 1.18
    @escape [type NodeId]
    @escape [type EdgeId]
    @section nodelegate
    /// edge identifier
    type EdgeId: Copy + PartialEq;
    /// node identifier
    type NodeId: Copy + PartialEq;
}
}

GraphBase! {delegate_impl []}
GraphBase! {delegate_impl [['a, G], G, &'a mut G, deref]}

/// A copyable reference to a graph.
pub trait GraphRef: Copy + GraphBase {}

impl<'a, G> GraphRef for &'a G where G: GraphBase {}

trait_template! {
/// Access to the neighbors of each node
///
/// The neighbors are, depending on the graph’s edge type:
///
/// - `Directed`: All targets of edges from `a`.
/// - `Undirected`: All other endpoints of edges connected to `a`.
pub trait IntoNeighbors : GraphRef {
    @section type
    type Neighbors: Iterator<Item=Self::NodeId>;
    @section self
    /// Return an iterator of the neighbors of node `a`.
    fn neighbors(self, a: Self::NodeId) -> Self::Neighbors;
}
}

IntoNeighbors! {delegate_impl []}

trait_template! {
/// Access to the neighbors of each node, through incoming or outgoing edges.
///
/// Depending on the graph’s edge type, the neighbors of a given directionality
/// are:
///
/// - `Directed`, `Outgoing`: All targets of edges from `a`.
/// - `Directed`, `Incoming`: All sources of edges to `a`.
/// - `Undirected`: All other endpoints of edges connected to `a`.
pub trait IntoNeighborsDirected : IntoNeighbors {
    @section type
    type NeighborsDirected: Iterator<Item=Self::NodeId>;
    @section self
    fn neighbors_directed(self, n: Self::NodeId, d: Direction)
        -> Self::NeighborsDirected;
}
}

trait_template! {
/// Access to the edges of each node.
///
/// The edges are, depending on the graph’s edge type:
///
/// - `Directed`: All edges from `a`.
/// - `Undirected`: All edges connected to `a`, with `a` being the source of each edge.
///
/// This is an extended version of the trait `IntoNeighbors`; the former
/// only iterates over the target node identifiers, while this trait
/// yields edge references (trait [`EdgeRef`][er]).
///
/// [er]: trait.EdgeRef.html
pub trait IntoEdges : IntoEdgeReferences + IntoNeighbors {
    @section type
    type Edges: Iterator<Item=Self::EdgeRef>;
    @section self
    fn edges(self, a: Self::NodeId) -> Self::Edges;
}
}

IntoEdges! {delegate_impl []}

trait_template! {
/// Access to all edges of each node, in the specified direction.
///
/// The edges are, depending on the direction and the graph’s edge type:
///
///
/// - `Directed`, `Outgoing`: All edges from `a`.
/// - `Directed`, `Incoming`: All edges to `a`.
/// - `Undirected`, `Outgoing`: All edges connected to `a`, with `a` being the source of each edge.
/// - `Undirected`, `Incoming`: All edges connected to `a`, with `a` being the target of each edge.
///
/// This is an extended version of the trait `IntoNeighborsDirected`; the former
/// only iterates over the target node identifiers, while this trait
/// yields edge references (trait [`EdgeRef`][er]).
///
/// [er]: trait.EdgeRef.html
pub trait IntoEdgesDirected : IntoEdges + IntoNeighborsDirected {
    @section type
    type EdgesDirected: Iterator<Item=Self::EdgeRef>;
    @section self
    fn edges_directed(self, a: Self::NodeId, dir: Direction) -> Self::EdgesDirected;
}
}

IntoEdgesDirected! {delegate_impl []}

trait_template! {
/// Access to the sequence of the graph’s `NodeId`s.
pub trait IntoNodeIdentifiers : GraphRef {
    @section type
    type NodeIdentifiers: Iterator<Item=Self::NodeId>;
    @section self
    fn node_identifiers(self) -> Self::NodeIdentifiers;
}
}

IntoNodeIdentifiers! {delegate_impl []}
IntoNeighborsDirected! {delegate_impl []}

trait_template! {
/// Define associated data for nodes and edges
pub trait Data : GraphBase {
    @section type
    type NodeWeight;
    type EdgeWeight;
}
}

Data! {delegate_impl []}
Data! {delegate_impl [['a, G], G, &'a mut G, deref]}

/// An edge reference.
///
/// Edge references are used by traits `IntoEdges` and `IntoEdgeReferences`.
pub trait EdgeRef: Copy {
    type NodeId;
    type EdgeId;
    type Weight;
    /// The source node of the edge.
    fn source(&self) -> Self::NodeId;
    /// The target node of the edge.
    fn target(&self) -> Self::NodeId;
    /// A reference to the weight of the edge.
    fn weight(&self) -> &Self::Weight;
    /// The edge’s identifier.
    fn id(&self) -> Self::EdgeId;
}

impl<'a, N, E> EdgeRef for (N, N, &'a E)
where
    N: Copy,
{
    type NodeId = N;
    type EdgeId = (N, N);
    type Weight = E;

    fn source(&self) -> N {
        self.0
    }
    fn target(&self) -> N {
        self.1
    }
    fn weight(&self) -> &E {
        self.2
    }
    fn id(&self) -> (N, N) {
        (self.0, self.1)
    }
}

/// A node reference.
pub trait NodeRef: Copy {
    type NodeId;
    type Weight;
    fn id(&self) -> Self::NodeId;
    fn weight(&self) -> &Self::Weight;
}

trait_template! {
/// Access to the sequence of the graph’s nodes
pub trait IntoNodeReferences : Data + IntoNodeIdentifiers {
    @section type
    type NodeRef: NodeRef<NodeId=Self::NodeId, Weight=Self::NodeWeight>;
    type NodeReferences: Iterator<Item=Self::NodeRef>;
    @section self
    fn node_references(self) -> Self::NodeReferences;
}
}

IntoNodeReferences! {delegate_impl []}

impl<Id> NodeRef for (Id, ())
where
    Id: Copy,
{
    type NodeId = Id;
    type Weight = ();
    fn id(&self) -> Self::NodeId {
        self.0
    }
    fn weight(&self) -> &Self::Weight {
        static DUMMY: () = ();
        &DUMMY
    }
}

impl<'a, Id, W> NodeRef for (Id, &'a W)
where
    Id: Copy,
{
    type NodeId = Id;
    type Weight = W;
    fn id(&self) -> Self::NodeId {
        self.0
    }
    fn weight(&self) -> &Self::Weight {
        self.1
    }
}

trait_template! {
/// Access to the sequence of the graph’s edges
pub trait IntoEdgeReferences : Data + GraphRef {
    @section type
    type EdgeRef: EdgeRef<NodeId=Self::NodeId, EdgeId=Self::EdgeId,
                          Weight=Self::EdgeWeight>;
    type EdgeReferences: Iterator<Item=Self::EdgeRef>;
    @section self
    fn edge_references(self) -> Self::EdgeReferences;
}
}

IntoEdgeReferences! {delegate_impl [] }

trait_template! {
    /// Edge kind property (directed or undirected edges)
pub trait GraphProp : GraphBase {
    @section type
    /// The kind of edges in the graph.
    type EdgeType: EdgeType;

    @section nodelegate
    fn is_directed(&self) -> bool {
        <Self::EdgeType>::is_directed()
    }
}
}

GraphProp! {delegate_impl []}

trait_template! {
    /// The graph’s `NodeId`s map to indices
    #[allow(clippy::needless_arbitrary_self_type)]
    pub trait NodeIndexable : GraphBase {
        @section self
        /// Return an upper bound of the node indices in the graph
        /// (suitable for the size of a bitmap).
        fn node_bound(self: &Self) -> usize;
        /// Convert `a` to an integer index.
        fn to_index(self: &Self, a: Self::NodeId) -> usize;
        /// Convert `i` to a node index. `i` must be a valid value in the graph.
        fn from_index(self: &Self, i: usize) -> Self::NodeId;
    }
}

NodeIndexable! {delegate_impl []}

trait_template! {
    /// The graph’s `NodeId`s map to indices
    #[allow(clippy::needless_arbitrary_self_type)]
    pub trait EdgeIndexable : GraphBase {
        @section self
        /// Return an upper bound of the edge indices in the graph
        /// (suitable for the size of a bitmap).
        fn edge_bound(self: &Self) -> usize;
        /// Convert `a` to an integer index.
        fn to_index(self: &Self, a: Self::EdgeId) -> usize;
        /// Convert `i` to an edge index. `i` must be a valid value in the graph.
        fn from_index(self: &Self, i: usize) -> Self::EdgeId;
    }
}

EdgeIndexable! {delegate_impl []}

trait_template! {
/// A graph with a known node count.
#[allow(clippy::needless_arbitrary_self_type)]
pub trait NodeCount : GraphBase {
    @section self
    fn node_count(self: &Self) -> usize;
}
}

NodeCount! {delegate_impl []}

trait_template! {
/// The graph’s `NodeId`s map to indices, in a range without holes.
///
/// The graph's node identifiers correspond to exactly the indices
/// `0..self.node_bound()`.
pub trait NodeCompactIndexable : NodeIndexable + NodeCount { }
}

NodeCompactIndexable! {delegate_impl []}

/// A mapping for storing the visited status for NodeId `N`.
pub trait VisitMap<N> {
    /// Mark `a` as visited.
    ///
    /// Return **true** if this is the first visit, false otherwise.
    fn visit(&mut self, a: N) -> bool;

    /// Return whether `a` has been visited before.
    fn is_visited(&self, a: &N) -> bool;
}

impl<Ix> VisitMap<Ix> for FixedBitSet
where
    Ix: IndexType,
{
    fn visit(&mut self, x: Ix) -> bool {
        !self.put(x.index())
    }
    fn is_visited(&self, x: &Ix) -> bool {
        self.contains(x.index())
    }
}

impl<N, S> VisitMap<N> for HashSet<N, S>
where
    N: Hash + Eq,
    S: BuildHasher,
{
    fn visit(&mut self, x: N) -> bool {
        self.insert(x)
    }
    fn is_visited(&self, x: &N) -> bool {
        self.contains(x)
    }
}

trait_template! {
/// A graph that can create a map that tracks the visited status of its nodes.
#[allow(clippy::needless_arbitrary_self_type)]
pub trait Visitable : GraphBase {
    @section type
    /// The associated map type
    type Map: VisitMap<Self::NodeId>;
    @section self
    /// Create a new visitor map
    fn visit_map(self: &Self) -> Self::Map;
    /// Reset the visitor map (and resize to new size of graph if needed)
    fn reset_map(self: &Self, map: &mut Self::Map);
}
}
Visitable! {delegate_impl []}

trait_template! {
/// Create or access the adjacency matrix of a graph.
///
/// The implementor can either create an adjacency matrix, or it can return
/// a placeholder if it has the needed representation internally.
#[allow(clippy::needless_arbitrary_self_type)]
pub trait GetAdjacencyMatrix : GraphBase {
    @section type
    /// The associated adjacency matrix type
    type AdjMatrix;
    @section self
    /// Create the adjacency matrix
    fn adjacency_matrix(self: &Self) -> Self::AdjMatrix;
    /// Return true if there is an edge from `a` to `b`, false otherwise.
    ///
    /// Computes in O(1) time.
    fn is_adjacent(self: &Self, matrix: &Self::AdjMatrix, a: Self::NodeId, b: Self::NodeId) -> bool;
}
}

GetAdjacencyMatrix! {delegate_impl []}

trait_template! {
/// A graph with a known edge count.
#[allow(clippy::needless_arbitrary_self_type)]
pub trait EdgeCount : GraphBase {
    @section self
    /// Return the number of edges in the graph.
    fn edge_count(self: &Self) -> usize;
}
}

EdgeCount! {delegate_impl []}

mod filter;
mod reversed;
