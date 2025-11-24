//! `petgraph` is a graph data structure library.
//!
//! Graphs are collections of nodes, and edges between nodes. `petgraph`
//! provides several [graph types](index.html#graph-types) (each differing in the
//! tradeoffs taken in their internal representation),
//! [algorithms](./algo/index.html#functions) on those graphs, and functionality to
//! [output graphs](./dot/struct.Dot.html) in
//! [`graphviz`](https://www.graphviz.org/) format. Both nodes and edges
//! can have arbitrary associated data, and edges may be either directed or undirected.
//!
//! # Example
//!
//! ```rust
//! use petgraph::graph::{NodeIndex, UnGraph};
//! use petgraph::algo::{dijkstra, min_spanning_tree};
//! use petgraph::data::FromElements;
//! use petgraph::dot::{Dot, Config};
//!
//! // Create an undirected graph with `i32` nodes and edges with `()` associated data.
//! let g = UnGraph::<i32, ()>::from_edges(&[
//!     (1, 2), (2, 3), (3, 4),
//!     (1, 4)]);
//!
//! // Find the shortest path from `1` to `4` using `1` as the cost for every edge.
//! let node_map = dijkstra(&g, 1.into(), Some(4.into()), |_| 1);
//! assert_eq!(&1i32, node_map.get(&NodeIndex::new(4)).unwrap());
//!
//! // Get the minimum spanning tree of the graph as a new graph, and check that
//! // one edge was trimmed.
//! let mst = UnGraph::<_, _>::from_elements(min_spanning_tree(&g));
//! assert_eq!(g.raw_edges().len() - 1, mst.raw_edges().len());
//!
//! // Output the tree to `graphviz` `DOT` format
//! println!("{:?}", Dot::with_config(&mst, &[Config::EdgeNoLabel]));
//! // graph {
//! //     0 [label="\"0\""]
//! //     1 [label="\"0\""]
//! //     2 [label="\"0\""]
//! //     3 [label="\"0\""]
//! //     1 -- 2
//! //     3 -- 4
//! //     2 -- 3
//! // }
//! ```
//!
//! # Graph types
//!
//! * [`Graph`](./graph/struct.Graph.html) -
//!   An adjacency list graph with arbitrary associated data.
//! * [`StableGraph`](./stable_graph/struct.StableGraph.html) -
//!   Similar to `Graph`, but it keeps indices stable across removals.
//! * [`GraphMap`](./graphmap/struct.GraphMap.html) -
//!   An adjacency list graph backed by a hash table. The node identifiers are the keys
//!   into the table.
//! * [`MatrixGraph`](./matrix_graph/struct.MatrixGraph.html) -
//!   An adjacency matrix graph.
//! * [`CSR`](./csr/struct.Csr.html) -
//!   A sparse adjacency matrix graph with arbitrary associated data.
//!
//! ### Generic parameters
//!
//! Each graph type is generic over a handful of parameters. All graphs share 3 common
//! parameters, `N`, `E`, and `Ty`. This is a broad overview of what those are. Each
//! type's documentation will have finer detail on these parameters.
//!
//! `N` & `E` are called *weights* in this implementation, and are associated with
//! nodes and edges respectively. They can generally be of arbitrary type, and don't have to
//! be what you might conventionally consider weight-like. For example, using `&str` for `N`
//! will work. Many algorithms that require costs let you provide a cost function that
//! translates your `N` and `E` weights into costs appropriate to the algorithm. Some graph
//! types and choices do impose bounds on `N` or `E`.
//! [`min_spanning_tree`](./algo/fn.min_spanning_tree.html) for example requires edge weights that
//! implement [`PartialOrd`](https://doc.rust-lang.org/stable/core/cmp/trait.PartialOrd.html).
//! [`GraphMap`](./graphmap/struct.GraphMap.html) requires node weights that can serve as hash
//! map keys, since that graph type does not create standalone node indices.
//!
//! `Ty` controls whether edges are [`Directed`](./enum.Directed.html) or
//! [`Undirected`](./enum.Undirected.html).
//!
//! `Ix` appears on graph types that use indices. It is exposed so you can control
//! the size of node and edge indices, and therefore the memory footprint of your graphs.
//! Allowed values are `u8`, `u16`, `u32`, and `usize`, with `u32` being the default.
//!
//! ### Shorthand types
//!
//! Each graph type vends a few shorthand type definitions that name some specific
//! generic choices. For example, [`DiGraph<_, _>`](./graph/type.DiGraph.html) is shorthand
//! for [`Graph<_, _, Directed>`](graph/struct.Graph.html).
//! [`UnMatrix<_, _>`](./matrix_graph/type.UnMatrix.html) is shorthand for
//! [`MatrixGraph<_, _, Undirected>`](./matrix_graph/struct.MatrixGraph.html). Each graph type's
//! module documentation lists the available shorthand types.
//!
//! # Crate features
//!
//! * **serde-1** -
//!   Defaults off. Enables serialization for ``Graph, StableGraph, GraphMap`` using
//!   [`serde 1.0`](https://crates.io/crates/serde). May require a more recent version
//!   of Rust than petgraph alone.
//! * **graphmap** -
//!   Defaults on. Enables [`GraphMap`](./graphmap/struct.GraphMap.html).
//! * **stable_graph** -
//!   Defaults on. Enables [`StableGraph`](./stable_graph/struct.StableGraph.html).
//! * **matrix_graph** -
//!   Defaults on. Enables [`MatrixGraph`](./matrix_graph/struct.MatrixGraph.html).
//!
#![doc(html_root_url = "https://docs.rs/petgraph/0.4/")]

extern crate fixedbitset;
#[cfg(feature = "graphmap")]
extern crate indexmap;

#[cfg(feature = "serde-1")]
extern crate serde;
#[cfg(feature = "serde-1")]
#[macro_use]
extern crate serde_derive;

#[cfg(all(feature = "serde-1", test))]
extern crate itertools;

#[doc(no_inline)]
pub use crate::graph::Graph;

pub use crate::Direction::{Incoming, Outgoing};

#[macro_use]
mod macros;
mod scored;

// these modules define trait-implementing macros
#[macro_use]
pub mod visit;
#[macro_use]
pub mod data;

pub mod adj;
pub mod algo;
pub mod csr;
pub mod dot;
#[cfg(feature = "generate")]
pub mod generate;
mod graph_impl;
#[cfg(feature = "graphmap")]
pub mod graphmap;
mod iter_format;
mod iter_utils;
#[cfg(feature = "matrix_graph")]
pub mod matrix_graph;
#[cfg(feature = "quickcheck")]
mod quickcheck;
#[cfg(feature = "serde-1")]
mod serde_utils;
mod traits_graph;
pub mod unionfind;
mod util;

pub mod operator;
pub mod prelude;

/// `Graph<N, E, Ty, Ix>` is a graph datastructure using an adjacency list representation.
pub mod graph {
    pub use crate::graph_impl::{
        edge_index, node_index, DefaultIx, DiGraph, Edge, EdgeIndex, EdgeIndices, EdgeReference,
        EdgeReferences, EdgeWeightsMut, Edges, EdgesConnecting, Externals, Frozen, Graph,
        GraphIndex, IndexType, Neighbors, Node, NodeIndex, NodeIndices, NodeReferences,
        NodeWeightsMut, UnGraph, WalkNeighbors,
    };
}

#[cfg(feature = "stable_graph")]
pub use crate::graph_impl::stable_graph;

// Index into the NodeIndex and EdgeIndex arrays
/// Edge direction.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[repr(usize)]
pub enum Direction {
    /// An `Outgoing` edge is an outward edge *from* the current node.
    Outgoing = 0,
    /// An `Incoming` edge is an inbound edge *to* the current node.
    Incoming = 1,
}

impl Direction {
    /// Return the opposite `Direction`.
    #[inline]
    pub fn opposite(self) -> Direction {
        match self {
            Outgoing => Incoming,
            Incoming => Outgoing,
        }
    }

    /// Return `0` for `Outgoing` and `1` for `Incoming`.
    #[inline]
    pub fn index(self) -> usize {
        (self as usize) & 0x1
    }
}

#[doc(hidden)]
pub use crate::Direction as EdgeDirection;

/// Marker type for a directed graph.
#[derive(Clone, Copy, Debug)]
pub enum Directed {}

/// Marker type for an undirected graph.
#[derive(Clone, Copy, Debug)]
pub enum Undirected {}

/// A graph's edge type determines whether it has directed edges or not.
pub trait EdgeType {
    fn is_directed() -> bool;
}

impl EdgeType for Directed {
    #[inline]
    fn is_directed() -> bool {
        true
    }
}

impl EdgeType for Undirected {
    #[inline]
    fn is_directed() -> bool {
        false
    }
}

/// Convert an element like `(i, j)` or `(i, j, w)` into
/// a triple of source, target, edge weight.
///
/// For `Graph::from_edges` and `GraphMap::from_edges`.
pub trait IntoWeightedEdge<E> {
    type NodeId;
    fn into_weighted_edge(self) -> (Self::NodeId, Self::NodeId, E);
}

impl<Ix, E> IntoWeightedEdge<E> for (Ix, Ix)
where
    E: Default,
{
    type NodeId = Ix;

    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        let (s, t) = self;
        (s, t, E::default())
    }
}

impl<Ix, E> IntoWeightedEdge<E> for (Ix, Ix, E) {
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        self
    }
}

impl<'a, Ix, E> IntoWeightedEdge<E> for (Ix, Ix, &'a E)
where
    E: Clone,
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        let (a, b, c) = self;
        (a, b, c.clone())
    }
}

impl<'a, Ix, E> IntoWeightedEdge<E> for &'a (Ix, Ix)
where
    Ix: Copy,
    E: Default,
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        let (s, t) = *self;
        (s, t, E::default())
    }
}

impl<'a, Ix, E> IntoWeightedEdge<E> for &'a (Ix, Ix, E)
where
    Ix: Copy,
    E: Clone,
{
    type NodeId = Ix;
    fn into_weighted_edge(self) -> (Ix, Ix, E) {
        self.clone()
    }
}
