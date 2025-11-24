//! Graph algorithms.
//!
//! It is a goal to gradually migrate the algorithms to be based on graph traits
//! so that they are generally applicable. For now, some of these still require
//! the `Graph` type.

pub mod astar;
pub mod bellman_ford;
pub mod dijkstra;
pub mod dominators;
pub mod feedback_arc_set;
pub mod floyd_warshall;
pub mod ford_fulkerson;
pub mod isomorphism;
pub mod k_shortest_path;
pub mod matching;
pub mod min_spanning_tree;
pub mod page_rank;
pub mod simple_paths;
pub mod tred;

use std::num::NonZeroUsize;

use crate::prelude::*;

use super::graph::IndexType;
use super::unionfind::UnionFind;
use super::visit::{
    GraphBase, GraphRef, IntoEdgeReferences, IntoNeighbors, IntoNeighborsDirected,
    IntoNodeIdentifiers, NodeCompactIndexable, NodeIndexable, Reversed, VisitMap, Visitable,
};
use super::EdgeType;
use crate::visit::Walker;

pub use astar::astar;
pub use bellman_ford::{bellman_ford, find_negative_cycle};
pub use dijkstra::dijkstra;
pub use feedback_arc_set::greedy_feedback_arc_set;
pub use floyd_warshall::floyd_warshall;
pub use ford_fulkerson::ford_fulkerson;
pub use isomorphism::{
    is_isomorphic, is_isomorphic_matching, is_isomorphic_subgraph, is_isomorphic_subgraph_matching,
    subgraph_isomorphisms_iter,
};
pub use k_shortest_path::k_shortest_path;
pub use matching::{greedy_matching, maximum_matching, Matching};
pub use min_spanning_tree::min_spanning_tree;
pub use page_rank::page_rank;
pub use simple_paths::all_simple_paths;

/// \[Generic\] Return the number of connected components of the graph.
///
/// For a directed graph, this is the *weakly* connected components.
/// # Example
/// ```rust
/// use petgraph::Graph;
/// use petgraph::algo::connected_components;
/// use petgraph::prelude::*;
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
///
/// graph.extend_with_edges(&[
///     (a, b),
///     (b, c),
///     (c, d),
///     (d, a),
///     (e, f),
///     (f, g),
///     (g, h),
///     (h, e)
/// ]);
/// // a ----> b       e ----> f
/// // ^       |       ^       |
/// // |       v       |       v
/// // d <---- c       h <---- g
///
/// assert_eq!(connected_components(&graph),2);
/// graph.add_edge(b,e,());
/// assert_eq!(connected_components(&graph),1);
/// ```
pub fn connected_components<G>(g: G) -> usize
where
    G: NodeCompactIndexable + IntoEdgeReferences,
{
    let mut vertex_sets = UnionFind::new(g.node_bound());
    for edge in g.edge_references() {
        let (a, b) = (edge.source(), edge.target());

        // union the two vertices of the edge
        vertex_sets.union(g.to_index(a), g.to_index(b));
    }
    let mut labels = vertex_sets.into_labeling();
    labels.sort_unstable();
    labels.dedup();
    labels.len()
}

/// \[Generic\] Return `true` if the input graph contains a cycle.
///
/// Always treats the input graph as if undirected.
pub fn is_cyclic_undirected<G>(g: G) -> bool
where
    G: NodeIndexable + IntoEdgeReferences,
{
    let mut edge_sets = UnionFind::new(g.node_bound());
    for edge in g.edge_references() {
        let (a, b) = (edge.source(), edge.target());

        // union the two vertices of the edge
        //  -- if they were already the same, then we have a cycle
        if !edge_sets.union(g.to_index(a), g.to_index(b)) {
            return true;
        }
    }
    false
}

/// \[Generic\] Perform a topological sort of a directed graph.
///
/// If the graph was acyclic, return a vector of nodes in topological order:
/// each node is ordered before its successors.
/// Otherwise, it will return a `Cycle` error. Self loops are also cycles.
///
/// To handle graphs with cycles, use the scc algorithms or `DfsPostOrder`
/// instead of this function.
///
/// If `space` is not `None`, it is used instead of creating a new workspace for
/// graph traversal. The implementation is iterative.
pub fn toposort<G>(
    g: G,
    space: Option<&mut DfsSpace<G::NodeId, G::Map>>,
) -> Result<Vec<G::NodeId>, Cycle<G::NodeId>>
where
    G: IntoNeighborsDirected + IntoNodeIdentifiers + Visitable,
{
    // based on kosaraju scc
    with_dfs(g, space, |dfs| {
        dfs.reset(g);
        let mut finished = g.visit_map();

        let mut finish_stack = Vec::new();
        for i in g.node_identifiers() {
            if dfs.discovered.is_visited(&i) {
                continue;
            }
            dfs.stack.push(i);
            while let Some(&nx) = dfs.stack.last() {
                if dfs.discovered.visit(nx) {
                    // First time visiting `nx`: Push neighbors, don't pop `nx`
                    for succ in g.neighbors(nx) {
                        if succ == nx {
                            // self cycle
                            return Err(Cycle(nx));
                        }
                        if !dfs.discovered.is_visited(&succ) {
                            dfs.stack.push(succ);
                        }
                    }
                } else {
                    dfs.stack.pop();
                    if finished.visit(nx) {
                        // Second time: All reachable nodes must have been finished
                        finish_stack.push(nx);
                    }
                }
            }
        }
        finish_stack.reverse();

        dfs.reset(g);
        for &i in &finish_stack {
            dfs.move_to(i);
            let mut cycle = false;
            while let Some(j) = dfs.next(Reversed(g)) {
                if cycle {
                    return Err(Cycle(j));
                }
                cycle = true;
            }
        }

        Ok(finish_stack)
    })
}

/// \[Generic\] Return `true` if the input directed graph contains a cycle.
///
/// This implementation is recursive; use `toposort` if an alternative is
/// needed.
pub fn is_cyclic_directed<G>(g: G) -> bool
where
    G: IntoNodeIdentifiers + IntoNeighbors + Visitable,
{
    use crate::visit::{depth_first_search, DfsEvent};

    depth_first_search(g, g.node_identifiers(), |event| match event {
        DfsEvent::BackEdge(_, _) => Err(()),
        _ => Ok(()),
    })
    .is_err()
}

type DfsSpaceType<G> = DfsSpace<<G as GraphBase>::NodeId, <G as Visitable>::Map>;

/// Workspace for a graph traversal.
#[derive(Clone, Debug)]
pub struct DfsSpace<N, VM> {
    dfs: Dfs<N, VM>,
}

impl<N, VM> DfsSpace<N, VM>
where
    N: Copy + PartialEq,
    VM: VisitMap<N>,
{
    pub fn new<G>(g: G) -> Self
    where
        G: GraphRef + Visitable<NodeId = N, Map = VM>,
    {
        DfsSpace { dfs: Dfs::empty(g) }
    }
}

impl<N, VM> Default for DfsSpace<N, VM>
where
    VM: VisitMap<N> + Default,
{
    fn default() -> Self {
        DfsSpace {
            dfs: Dfs {
                stack: <_>::default(),
                discovered: <_>::default(),
            },
        }
    }
}

/// Create a Dfs if it's needed
fn with_dfs<G, F, R>(g: G, space: Option<&mut DfsSpaceType<G>>, f: F) -> R
where
    G: GraphRef + Visitable,
    F: FnOnce(&mut Dfs<G::NodeId, G::Map>) -> R,
{
    let mut local_visitor;
    let dfs = if let Some(v) = space {
        &mut v.dfs
    } else {
        local_visitor = Dfs::empty(g);
        &mut local_visitor
    };
    f(dfs)
}

/// \[Generic\] Check if there exists a path starting at `from` and reaching `to`.
///
/// If `from` and `to` are equal, this function returns true.
///
/// If `space` is not `None`, it is used instead of creating a new workspace for
/// graph traversal.
pub fn has_path_connecting<G>(
    g: G,
    from: G::NodeId,
    to: G::NodeId,
    space: Option<&mut DfsSpace<G::NodeId, G::Map>>,
) -> bool
where
    G: IntoNeighbors + Visitable,
{
    with_dfs(g, space, |dfs| {
        dfs.reset(g);
        dfs.move_to(from);
        dfs.iter(g).any(|x| x == to)
    })
}

/// Renamed to `kosaraju_scc`.
#[deprecated(note = "renamed to kosaraju_scc")]
pub fn scc<G>(g: G) -> Vec<Vec<G::NodeId>>
where
    G: IntoNeighborsDirected + Visitable + IntoNodeIdentifiers,
{
    kosaraju_scc(g)
}

/// \[Generic\] Compute the *strongly connected components* using [Kosaraju's algorithm][1].
///
/// [1]: https://en.wikipedia.org/wiki/Kosaraju%27s_algorithm
///
/// Return a vector where each element is a strongly connected component (scc).
/// The order of node ids within each scc is arbitrary, but the order of
/// the sccs is their postorder (reverse topological sort).
///
/// For an undirected graph, the sccs are simply the connected components.
///
/// This implementation is iterative and does two passes over the nodes.
pub fn kosaraju_scc<G>(g: G) -> Vec<Vec<G::NodeId>>
where
    G: IntoNeighborsDirected + Visitable + IntoNodeIdentifiers,
{
    let mut dfs = DfsPostOrder::empty(g);

    // First phase, reverse dfs pass, compute finishing times.
    // http://stackoverflow.com/a/26780899/161659
    let mut finish_order = Vec::with_capacity(0);
    for i in g.node_identifiers() {
        if dfs.discovered.is_visited(&i) {
            continue;
        }

        dfs.move_to(i);
        while let Some(nx) = dfs.next(Reversed(g)) {
            finish_order.push(nx);
        }
    }

    let mut dfs = Dfs::from_parts(dfs.stack, dfs.discovered);
    dfs.reset(g);
    let mut sccs = Vec::new();

    // Second phase
    // Process in decreasing finishing time order
    for i in finish_order.into_iter().rev() {
        if dfs.discovered.is_visited(&i) {
            continue;
        }
        // Move to the leader node `i`.
        dfs.move_to(i);
        let mut scc = Vec::new();
        while let Some(nx) = dfs.next(g) {
            scc.push(nx);
        }
        sccs.push(scc);
    }
    sccs
}

#[derive(Copy, Clone, Debug)]
struct NodeData {
    rootindex: Option<NonZeroUsize>,
}

/// A reusable state for computing the *strongly connected components* using [Tarjan's algorithm][1].
///
/// [1]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
#[derive(Debug)]
pub struct TarjanScc<N> {
    index: usize,
    componentcount: usize,
    nodes: Vec<NodeData>,
    stack: Vec<N>,
}

impl<N> Default for TarjanScc<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N> TarjanScc<N> {
    /// Creates a new `TarjanScc`
    pub fn new() -> Self {
        TarjanScc {
            index: 1,                        // Invariant: index < componentcount at all times.
            componentcount: std::usize::MAX, // Will hold if componentcount is initialized to number of nodes - 1 or higher.
            nodes: Vec::new(),
            stack: Vec::new(),
        }
    }

    /// \[Generic\] Compute the *strongly connected components* using Algorithm 3 in
    /// [A Space-Efficient Algorithm for Finding Strongly Connected Components][1] by David J. Pierce,
    /// which is a memory-efficient variation of [Tarjan's algorithm][2].
    ///
    ///
    /// [1]: https://homepages.ecs.vuw.ac.nz/~djp/files/P05.pdf
    /// [2]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
    ///
    /// Calls `f` for each strongly strongly connected component (scc).
    /// The order of node ids within each scc is arbitrary, but the order of
    /// the sccs is their postorder (reverse topological sort).
    ///
    /// For an undirected graph, the sccs are simply the connected components.
    ///
    /// This implementation is recursive and does one pass over the nodes.
    pub fn run<G, F>(&mut self, g: G, mut f: F)
    where
        G: IntoNodeIdentifiers<NodeId = N> + IntoNeighbors<NodeId = N> + NodeIndexable<NodeId = N>,
        F: FnMut(&[N]),
        N: Copy + PartialEq,
    {
        self.nodes.clear();
        self.nodes
            .resize(g.node_bound(), NodeData { rootindex: None });

        for n in g.node_identifiers() {
            let visited = self.nodes[g.to_index(n)].rootindex.is_some();
            if !visited {
                self.visit(n, g, &mut f);
            }
        }

        debug_assert!(self.stack.is_empty());
    }

    fn visit<G, F>(&mut self, v: G::NodeId, g: G, f: &mut F)
    where
        G: IntoNeighbors<NodeId = N> + NodeIndexable<NodeId = N>,
        F: FnMut(&[N]),
        N: Copy + PartialEq,
    {
        macro_rules! node {
            ($node:expr) => {
                self.nodes[g.to_index($node)]
            };
        }

        let node_v = &mut node![v];
        debug_assert!(node_v.rootindex.is_none());

        let mut v_is_local_root = true;
        let v_index = self.index;
        node_v.rootindex = NonZeroUsize::new(v_index);
        self.index += 1;

        for w in g.neighbors(v) {
            if node![w].rootindex.is_none() {
                self.visit(w, g, f);
            }
            if node![w].rootindex < node![v].rootindex {
                node![v].rootindex = node![w].rootindex;
                v_is_local_root = false
            }
        }

        if v_is_local_root {
            // Pop the stack and generate an SCC.
            let mut indexadjustment = 1;
            let c = NonZeroUsize::new(self.componentcount);
            let nodes = &mut self.nodes;
            let start = self
                .stack
                .iter()
                .rposition(|&w| {
                    if nodes[g.to_index(v)].rootindex > nodes[g.to_index(w)].rootindex {
                        true
                    } else {
                        nodes[g.to_index(w)].rootindex = c;
                        indexadjustment += 1;
                        false
                    }
                })
                .map(|x| x + 1)
                .unwrap_or_default();
            nodes[g.to_index(v)].rootindex = c;
            self.stack.push(v); // Pushing the component root to the back right before getting rid of it is somewhat ugly, but it lets it be included in f.
            f(&self.stack[start..]);
            self.stack.truncate(start);
            self.index -= indexadjustment; // Backtrack index back to where it was before we ever encountered the component.
            self.componentcount -= 1;
        } else {
            self.stack.push(v); // Stack is filled up when backtracking, unlike in Tarjans original algorithm.
        }
    }

    /// Returns the index of the component in which v has been assigned. Allows for using self as a lookup table for an scc decomposition produced by self.run().
    pub fn node_component_index<G>(&self, g: G, v: N) -> usize
    where
        G: IntoNeighbors<NodeId = N> + NodeIndexable<NodeId = N>,
        N: Copy + PartialEq,
    {
        let rindex: usize = self.nodes[g.to_index(v)]
            .rootindex
            .map(NonZeroUsize::get)
            .unwrap_or(0); // Compiles to no-op.
        debug_assert!(
            rindex != 0,
            "Tried to get the component index of an unvisited node."
        );
        debug_assert!(
            rindex > self.componentcount,
            "Given node has been visited but not yet assigned to a component."
        );
        std::usize::MAX - rindex
    }
}

/// \[Generic\] Compute the *strongly connected components* using [Tarjan's algorithm][1].
///
/// [1]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
/// [2]: https://homepages.ecs.vuw.ac.nz/~djp/files/P05.pdf
///
/// Return a vector where each element is a strongly connected component (scc).
/// The order of node ids within each scc is arbitrary, but the order of
/// the sccs is their postorder (reverse topological sort).
///
/// For an undirected graph, the sccs are simply the connected components.
///
/// This implementation is recursive and does one pass over the nodes. It is based on
/// [A Space-Efficient Algorithm for Finding Strongly Connected Components][2] by David J. Pierce,
/// to provide a memory-efficient implementation of [Tarjan's algorithm][1].
pub fn tarjan_scc<G>(g: G) -> Vec<Vec<G::NodeId>>
where
    G: IntoNodeIdentifiers + IntoNeighbors + NodeIndexable,
{
    let mut sccs = Vec::new();
    {
        let mut tarjan_scc = TarjanScc::new();
        tarjan_scc.run(g, |scc| sccs.push(scc.to_vec()));
    }
    sccs
}

/// [Graph] Condense every strongly connected component into a single node and return the result.
///
/// If `make_acyclic` is true, self-loops and multi edges are ignored, guaranteeing that
/// the output is acyclic.
/// # Example
/// ```rust
/// use petgraph::Graph;
/// use petgraph::algo::condensation;
/// use petgraph::prelude::*;
///
/// let mut graph : Graph<(),(),Directed> = Graph::new();
/// let a = graph.add_node(()); // node with no weight
/// let b = graph.add_node(());
/// let c = graph.add_node(());
/// let d = graph.add_node(());
/// let e = graph.add_node(());
/// let f = graph.add_node(());
/// let g = graph.add_node(());
/// let h = graph.add_node(());
///
/// graph.extend_with_edges(&[
///     (a, b),
///     (b, c),
///     (c, d),
///     (d, a),
///     (b, e),
///     (e, f),
///     (f, g),
///     (g, h),
///     (h, e)
/// ]);
///
/// // a ----> b ----> e ----> f
/// // ^       |       ^       |
/// // |       v       |       v
/// // d <---- c       h <---- g
///
/// let condensed_graph = condensation(graph,false);
/// let A = NodeIndex::new(0);
/// let B = NodeIndex::new(1);
/// assert_eq!(condensed_graph.node_count(), 2);
/// assert_eq!(condensed_graph.edge_count(), 9);
/// assert_eq!(condensed_graph.neighbors(A).collect::<Vec<_>>(), vec![A, A, A, A]);
/// assert_eq!(condensed_graph.neighbors(B).collect::<Vec<_>>(), vec![A, B, B, B, B]);
/// ```
/// If `make_acyclic` is true, self-loops and multi edges are ignored:
///
/// ```rust
/// # use petgraph::Graph;
/// # use petgraph::algo::condensation;
/// # use petgraph::prelude::*;
/// #
/// # let mut graph : Graph<(),(),Directed> = Graph::new();
/// # let a = graph.add_node(()); // node with no weight
/// # let b = graph.add_node(());
/// # let c = graph.add_node(());
/// # let d = graph.add_node(());
/// # let e = graph.add_node(());
/// # let f = graph.add_node(());
/// # let g = graph.add_node(());
/// # let h = graph.add_node(());
/// #
/// # graph.extend_with_edges(&[
/// #    (a, b),
/// #    (b, c),
/// #    (c, d),
/// #    (d, a),
/// #    (b, e),
/// #    (e, f),
/// #    (f, g),
/// #    (g, h),
/// #    (h, e)
/// # ]);
/// let acyclic_condensed_graph = condensation(graph, true);
/// let A = NodeIndex::new(0);
/// let B = NodeIndex::new(1);
/// assert_eq!(acyclic_condensed_graph.node_count(), 2);
/// assert_eq!(acyclic_condensed_graph.edge_count(), 1);
/// assert_eq!(acyclic_condensed_graph.neighbors(B).collect::<Vec<_>>(), vec![A]);
/// ```
pub fn condensation<N, E, Ty, Ix>(
    g: Graph<N, E, Ty, Ix>,
    make_acyclic: bool,
) -> Graph<Vec<N>, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType,
{
    let sccs = kosaraju_scc(&g);
    let mut condensed: Graph<Vec<N>, E, Ty, Ix> = Graph::with_capacity(sccs.len(), g.edge_count());

    // Build a map from old indices to new ones.
    let mut node_map = vec![NodeIndex::end(); g.node_count()];
    for comp in sccs {
        let new_nix = condensed.add_node(Vec::new());
        for nix in comp {
            node_map[nix.index()] = new_nix;
        }
    }

    // Consume nodes and edges of the old graph and insert them into the new one.
    let (nodes, edges) = g.into_nodes_edges();
    for (nix, node) in nodes.into_iter().enumerate() {
        condensed[node_map[nix]].push(node.weight);
    }
    for edge in edges {
        let source = node_map[edge.source().index()];
        let target = node_map[edge.target().index()];
        if make_acyclic {
            if source != target {
                condensed.update_edge(source, target, edge.weight);
            }
        } else {
            condensed.add_edge(source, target, edge.weight);
        }
    }
    condensed
}

/// An algorithm error: a cycle was found in the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Cycle<N>(N);

impl<N> Cycle<N> {
    /// Return a node id that participates in the cycle
    pub fn node_id(&self) -> N
    where
        N: Copy,
    {
        self.0
    }
}

/// An algorithm error: a cycle of negative weights was found in the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct NegativeCycle(pub ());

/// Return `true` if the graph is bipartite. A graph is bipartite if its nodes can be divided into
/// two disjoint and indepedent sets U and V such that every edge connects U to one in V. This
/// algorithm implements 2-coloring algorithm based on the BFS algorithm.
///
/// Always treats the input graph as if undirected.
pub fn is_bipartite_undirected<G, N, VM>(g: G, start: N) -> bool
where
    G: GraphRef + Visitable<NodeId = N, Map = VM> + IntoNeighbors<NodeId = N>,
    N: Copy + PartialEq + std::fmt::Debug,
    VM: VisitMap<N>,
{
    let mut red = g.visit_map();
    red.visit(start);
    let mut blue = g.visit_map();

    let mut stack = ::std::collections::VecDeque::new();
    stack.push_front(start);

    while let Some(node) = stack.pop_front() {
        let is_red = red.is_visited(&node);
        let is_blue = blue.is_visited(&node);

        assert!(is_red ^ is_blue);

        for neighbour in g.neighbors(node) {
            let is_neigbour_red = red.is_visited(&neighbour);
            let is_neigbour_blue = blue.is_visited(&neighbour);

            if (is_red && is_neigbour_red) || (is_blue && is_neigbour_blue) {
                return false;
            }

            if !is_neigbour_red && !is_neigbour_blue {
                //hasn't been visited yet

                match (is_red, is_blue) {
                    (true, false) => {
                        blue.visit(neighbour);
                    }
                    (false, true) => {
                        red.visit(neighbour);
                    }
                    (_, _) => {
                        panic!("Invariant doesn't hold");
                    }
                }

                stack.push_back(neighbour);
            }
        }
    }

    true
}

use std::fmt::Debug;
use std::ops::Add;

/// Associated data that can be used for measures (such as length).
pub trait Measure: Debug + PartialOrd + Add<Self, Output = Self> + Default + Clone {}

impl<M> Measure for M where M: Debug + PartialOrd + Add<M, Output = M> + Default + Clone {}

/// A floating-point measure.
pub trait FloatMeasure: Measure + Copy {
    fn zero() -> Self;
    fn infinite() -> Self;
}

impl FloatMeasure for f32 {
    fn zero() -> Self {
        0.
    }
    fn infinite() -> Self {
        1. / 0.
    }
}

impl FloatMeasure for f64 {
    fn zero() -> Self {
        0.
    }
    fn infinite() -> Self {
        1. / 0.
    }
}

pub trait BoundedMeasure: Measure + std::ops::Sub<Self, Output = Self> {
    fn min() -> Self;
    fn max() -> Self;
    fn overflowing_add(self, rhs: Self) -> (Self, bool);
}

macro_rules! impl_bounded_measure_integer(
    ( $( $t:ident ),* ) => {
        $(
            impl BoundedMeasure for $t {
                fn min() -> Self {
                    std::$t::MIN
                }

                fn max() -> Self {
                    std::$t::MAX
                }

                fn overflowing_add(self, rhs: Self) -> (Self, bool) {
                    self.overflowing_add(rhs)
                }
            }
        )*
    };
);

impl_bounded_measure_integer!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

macro_rules! impl_bounded_measure_float(
    ( $( $t:ident ),* ) => {
        $(
            impl BoundedMeasure for $t {
                fn min() -> Self {
                    std::$t::MIN
                }

                fn max() -> Self {
                    std::$t::MAX
                }

                fn overflowing_add(self, rhs: Self) -> (Self, bool) {
                    // for an overflow: a + b > max: both values need to be positive and a > max - b must be satisfied
                    let overflow = self > Self::default() && rhs > Self::default() && self > std::$t::MAX - rhs;

                    // for an underflow: a + b < min: overflow can not happen and both values must be negative and a < min - b must be satisfied
                    let underflow = !overflow && self < Self::default() && rhs < Self::default() && self < std::$t::MIN - rhs;

                    (self + rhs, overflow || underflow)
                }
            }
        )*
    };
);

impl_bounded_measure_float!(f32, f64);

/// A floating-point measure that can be computed from `usize`
/// and with a default measure of proximity.  
pub trait UnitMeasure:
    Measure
    + std::ops::Sub<Self, Output = Self>
    + std::ops::Mul<Self, Output = Self>
    + std::ops::Div<Self, Output = Self>
    + std::iter::Sum
{
    fn zero() -> Self;
    fn one() -> Self;
    fn from_usize(nb: usize) -> Self;
    fn default_tol() -> Self;
}

macro_rules! impl_unit_measure(
    ( $( $t:ident ),* )=> {
        $(
            impl UnitMeasure for $t {
                fn zero() -> Self {
                    0 as $t
                }
                fn one() -> Self {
                    1 as $t
                }

                fn from_usize(nb: usize) -> Self {
                    nb as $t
                }

                fn default_tol() -> Self {
                    1e-6 as $t
                }

            }

        )*
    }
);
impl_unit_measure!(f32, f64);

/// Some measure of positive numbers, assuming positive
/// float-pointing numbers
pub trait PositiveMeasure: Measure + Copy {
    fn zero() -> Self;
    fn max() -> Self;
}

macro_rules! impl_positive_measure(
    ( $( $t:ident ),* )=> {
        $(
            impl PositiveMeasure for $t {
                fn zero() -> Self {
                    0 as $t
                }
                fn max() -> Self {
                    std::$t::MAX
                }
            }

        )*
    }
);

impl_positive_measure!(u8, u16, u32, u64, u128, usize, f32, f64);
