//! Graph traits for associated data and graph construction.

use crate::graph::IndexType;
#[cfg(feature = "graphmap")]
use crate::graphmap::{GraphMap, NodeTrait};
#[cfg(feature = "stable_graph")]
use crate::stable_graph::StableGraph;
use crate::visit::{Data, NodeCount, NodeIndexable, Reversed};
use crate::EdgeType;
use crate::Graph;

trait_template! {
    /// Access node and edge weights (associated data).
#[allow(clippy::needless_arbitrary_self_type)]
pub trait DataMap : Data {
    @section self
    fn node_weight(self: &Self, id: Self::NodeId) -> Option<&Self::NodeWeight>;
    fn edge_weight(self: &Self, id: Self::EdgeId) -> Option<&Self::EdgeWeight>;
}
}

macro_rules! access0 {
    ($e:expr) => {
        $e.0
    };
}

DataMap! {delegate_impl []}
DataMap! {delegate_impl [['a, G], G, &'a mut G, deref_twice]}
DataMap! {delegate_impl [[G], G, Reversed<G>, access0]}

trait_template! {
    /// Access node and edge weights mutably.
#[allow(clippy::needless_arbitrary_self_type)]
pub trait DataMapMut : DataMap {
    @section self
    fn node_weight_mut(self: &mut Self, id: Self::NodeId) -> Option<&mut Self::NodeWeight>;
    fn edge_weight_mut(self: &mut Self, id: Self::EdgeId) -> Option<&mut Self::EdgeWeight>;
}
}

DataMapMut! {delegate_impl [['a, G], G, &'a mut G, deref_twice]}
DataMapMut! {delegate_impl [[G], G, Reversed<G>, access0]}

/// A graph that can be extended with further nodes and edges
pub trait Build: Data + NodeCount {
    fn add_node(&mut self, weight: Self::NodeWeight) -> Self::NodeId;
    /// Add a new edge. If parallel edges (duplicate) are not allowed and
    /// the edge already exists, return `None`.
    fn add_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Option<Self::EdgeId> {
        Some(self.update_edge(a, b, weight))
    }
    /// Add or update the edge from `a` to `b`. Return the id of the affected
    /// edge.
    fn update_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Self::EdgeId;
}

/// A graph that can be created
pub trait Create: Build + Default {
    fn with_capacity(nodes: usize, edges: usize) -> Self;
}

impl<N, E, Ty, Ix> Data for Graph<N, E, Ty, Ix>
where
    Ix: IndexType,
{
    type NodeWeight = N;
    type EdgeWeight = E;
}

impl<N, E, Ty, Ix> DataMap for Graph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn node_weight(&self, id: Self::NodeId) -> Option<&Self::NodeWeight> {
        self.node_weight(id)
    }
    fn edge_weight(&self, id: Self::EdgeId) -> Option<&Self::EdgeWeight> {
        self.edge_weight(id)
    }
}

impl<N, E, Ty, Ix> DataMapMut for Graph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn node_weight_mut(&mut self, id: Self::NodeId) -> Option<&mut Self::NodeWeight> {
        self.node_weight_mut(id)
    }
    fn edge_weight_mut(&mut self, id: Self::EdgeId) -> Option<&mut Self::EdgeWeight> {
        self.edge_weight_mut(id)
    }
}

#[cfg(feature = "stable_graph")]
impl<N, E, Ty, Ix> DataMap for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn node_weight(&self, id: Self::NodeId) -> Option<&Self::NodeWeight> {
        self.node_weight(id)
    }
    fn edge_weight(&self, id: Self::EdgeId) -> Option<&Self::EdgeWeight> {
        self.edge_weight(id)
    }
}

#[cfg(feature = "stable_graph")]
impl<N, E, Ty, Ix> DataMapMut for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn node_weight_mut(&mut self, id: Self::NodeId) -> Option<&mut Self::NodeWeight> {
        self.node_weight_mut(id)
    }
    fn edge_weight_mut(&mut self, id: Self::EdgeId) -> Option<&mut Self::EdgeWeight> {
        self.edge_weight_mut(id)
    }
}

impl<N, E, Ty, Ix> Build for Graph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn add_node(&mut self, weight: Self::NodeWeight) -> Self::NodeId {
        self.add_node(weight)
    }
    fn add_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Option<Self::EdgeId> {
        Some(self.add_edge(a, b, weight))
    }
    fn update_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Self::EdgeId {
        self.update_edge(a, b, weight)
    }
}

#[cfg(feature = "stable_graph")]
impl<N, E, Ty, Ix> Build for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn add_node(&mut self, weight: Self::NodeWeight) -> Self::NodeId {
        self.add_node(weight)
    }
    fn add_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Option<Self::EdgeId> {
        Some(self.add_edge(a, b, weight))
    }
    fn update_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Self::EdgeId {
        self.update_edge(a, b, weight)
    }
}

#[cfg(feature = "graphmap")]
impl<N, E, Ty> Build for GraphMap<N, E, Ty>
where
    Ty: EdgeType,
    N: NodeTrait,
{
    fn add_node(&mut self, weight: Self::NodeWeight) -> Self::NodeId {
        self.add_node(weight)
    }
    fn add_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Option<Self::EdgeId> {
        if self.contains_edge(a, b) {
            None
        } else {
            let r = self.add_edge(a, b, weight);
            debug_assert!(r.is_none());
            Some((a, b))
        }
    }
    fn update_edge(
        &mut self,
        a: Self::NodeId,
        b: Self::NodeId,
        weight: Self::EdgeWeight,
    ) -> Self::EdgeId {
        self.add_edge(a, b, weight);
        (a, b)
    }
}

impl<N, E, Ty, Ix> Create for Graph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self::with_capacity(nodes, edges)
    }
}

#[cfg(feature = "stable_graph")]
impl<N, E, Ty, Ix> Create for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self::with_capacity(nodes, edges)
    }
}

#[cfg(feature = "graphmap")]
impl<N, E, Ty> Create for GraphMap<N, E, Ty>
where
    Ty: EdgeType,
    N: NodeTrait,
{
    fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self::with_capacity(nodes, edges)
    }
}

/// A graph element.
///
/// A sequence of Elements, for example an iterator, is laid out as follows:
/// Nodes are implicitly given the index of their appearance in the sequence.
/// The edges’ source and target fields refer to these indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Element<N, E> {
    /// A graph node.
    Node { weight: N },
    /// A graph edge.
    Edge {
        source: usize,
        target: usize,
        weight: E,
    },
}

/// Create a graph from an iterator of elements.
pub trait FromElements: Create {
    fn from_elements<I>(iterable: I) -> Self
    where
        Self: Sized,
        I: IntoIterator<Item = Element<Self::NodeWeight, Self::EdgeWeight>>,
    {
        let mut gr = Self::with_capacity(0, 0);
        // usize -> NodeId map
        let mut map = Vec::new();
        for element in iterable {
            match element {
                Element::Node { weight } => {
                    map.push(gr.add_node(weight));
                }
                Element::Edge {
                    source,
                    target,
                    weight,
                } => {
                    gr.add_edge(map[source], map[target], weight);
                }
            }
        }
        gr
    }
}

fn from_elements_indexable<G, I>(iterable: I) -> G
where
    G: Create + NodeIndexable,
    I: IntoIterator<Item = Element<G::NodeWeight, G::EdgeWeight>>,
{
    let mut gr = G::with_capacity(0, 0);
    let map = |gr: &G, i| gr.from_index(i);
    for element in iterable {
        match element {
            Element::Node { weight } => {
                gr.add_node(weight);
            }
            Element::Edge {
                source,
                target,
                weight,
            } => {
                let from = map(&gr, source);
                let to = map(&gr, target);
                gr.add_edge(from, to, weight);
            }
        }
    }
    gr
}

impl<N, E, Ty, Ix> FromElements for Graph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn from_elements<I>(iterable: I) -> Self
    where
        Self: Sized,
        I: IntoIterator<Item = Element<Self::NodeWeight, Self::EdgeWeight>>,
    {
        from_elements_indexable(iterable)
    }
}

#[cfg(feature = "stable_graph")]
impl<N, E, Ty, Ix> FromElements for StableGraph<N, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    fn from_elements<I>(iterable: I) -> Self
    where
        Self: Sized,
        I: IntoIterator<Item = Element<Self::NodeWeight, Self::EdgeWeight>>,
    {
        from_elements_indexable(iterable)
    }
}

#[cfg(feature = "graphmap")]
impl<N, E, Ty> FromElements for GraphMap<N, E, Ty>
where
    Ty: EdgeType,
    N: NodeTrait,
{
    fn from_elements<I>(iterable: I) -> Self
    where
        Self: Sized,
        I: IntoIterator<Item = Element<Self::NodeWeight, Self::EdgeWeight>>,
    {
        from_elements_indexable(iterable)
    }
}

/// Iterator adaptors for iterators of `Element`.
pub trait ElementIterator<N, E>: Iterator<Item = Element<N, E>> {
    /// Create an iterator adaptor that filters graph elements.
    ///
    /// The function `f` is called with each element and if its return value
    /// is `true` the element is accepted and if `false` it is removed.
    /// `f` is called with mutable references to the node and edge weights,
    /// so that they can be mutated (but the edge endpoints can not).
    ///
    /// This filter adapts the edge source and target indices in the
    /// stream so that they are correct after the removals.
    fn filter_elements<F>(self, f: F) -> FilterElements<Self, F>
    where
        Self: Sized,
        F: FnMut(Element<&mut N, &mut E>) -> bool,
    {
        FilterElements {
            iter: self,
            node_index: 0,
            map: Vec::new(),
            f,
        }
    }
}

impl<N, E, I: ?Sized> ElementIterator<N, E> for I where I: Iterator<Item = Element<N, E>> {}

/// An iterator that filters graph elements.
///
/// See [`.filter_elements()`][1] for more information.
///
/// [1]: trait.ElementIterator.html#method.filter_elements
#[derive(Debug, Clone)]
pub struct FilterElements<I, F> {
    iter: I,
    node_index: usize,
    map: Vec<usize>,
    f: F,
}

impl<I, F, N, E> Iterator for FilterElements<I, F>
where
    I: Iterator<Item = Element<N, E>>,
    F: FnMut(Element<&mut N, &mut E>) -> bool,
{
    type Item = Element<N, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut elt = match self.iter.next() {
                None => return None,
                Some(elt) => elt,
            };
            let keep = (self.f)(match elt {
                Element::Node { ref mut weight } => Element::Node { weight },
                Element::Edge {
                    source,
                    target,
                    ref mut weight,
                } => Element::Edge {
                    source,
                    target,
                    weight,
                },
            });
            let is_node = if let Element::Node { .. } = elt {
                true
            } else {
                false
            };
            if !keep && is_node {
                self.map.push(self.node_index);
            }
            if is_node {
                self.node_index += 1;
            }
            if !keep {
                continue;
            }

            // map edge parts
            match elt {
                Element::Edge {
                    ref mut source,
                    ref mut target,
                    ..
                } => {
                    // Find the node indices in the map of removed ones.
                    // If a node was removed, the edge is as well.
                    // Otherwise the counts are adjusted by the number of nodes
                    // removed.
                    // Example: map: [1, 3, 4, 6]
                    // binary search for 2, result is Err(1). One node has been
                    // removed before 2.
                    match self.map.binary_search(source) {
                        Ok(_) => continue,
                        Err(i) => *source -= i,
                    }
                    match self.map.binary_search(target) {
                        Ok(_) => continue,
                        Err(i) => *target -= i,
                    }
                }
                Element::Node { .. } => {}
            }
            return Some(elt);
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}
