use std::collections::VecDeque;
use std::hash::Hash;

use crate::visit::{
    EdgeRef, GraphBase, IntoEdges, IntoNeighbors, IntoNodeIdentifiers, NodeCount, NodeIndexable,
    VisitMap, Visitable,
};

/// Computed
/// [*matching*](https://en.wikipedia.org/wiki/Matching_(graph_theory)#Definitions)
/// of the graph.
pub struct Matching<G: GraphBase> {
    graph: G,
    mate: Vec<Option<G::NodeId>>,
    n_edges: usize,
}

impl<G> Matching<G>
where
    G: GraphBase,
{
    fn new(graph: G, mate: Vec<Option<G::NodeId>>, n_edges: usize) -> Self {
        Self {
            graph,
            mate,
            n_edges,
        }
    }
}

impl<G> Matching<G>
where
    G: NodeIndexable,
{
    /// Gets the matched counterpart of given node, if there is any.
    ///
    /// Returns `None` if the node is not matched or does not exist.
    pub fn mate(&self, node: G::NodeId) -> Option<G::NodeId> {
        self.mate.get(self.graph.to_index(node)).and_then(|&id| id)
    }

    /// Iterates over all edges from the matching.
    ///
    /// An edge is represented by its endpoints. The graph is considered
    /// undirected and every pair of matched nodes is reported only once.
    pub fn edges(&self) -> MatchedEdges<'_, G> {
        MatchedEdges {
            graph: &self.graph,
            mate: self.mate.as_slice(),
            current: 0,
        }
    }

    /// Iterates over all nodes from the matching.
    pub fn nodes(&self) -> MatchedNodes<'_, G> {
        MatchedNodes {
            graph: &self.graph,
            mate: self.mate.as_slice(),
            current: 0,
        }
    }

    /// Returns `true` if given edge is in the matching, or `false` otherwise.
    ///
    /// If any of the nodes does not exist, `false` is returned.
    pub fn contains_edge(&self, a: G::NodeId, b: G::NodeId) -> bool {
        match self.mate(a) {
            Some(mate) => mate == b,
            None => false,
        }
    }

    /// Returns `true` if given node is in the matching, or `false` otherwise.
    ///
    /// If the node does not exist, `false` is returned.
    pub fn contains_node(&self, node: G::NodeId) -> bool {
        self.mate(node).is_some()
    }

    /// Gets the number of matched **edges**.
    pub fn len(&self) -> usize {
        self.n_edges
    }

    /// Returns `true` if the number of matched **edges** is 0.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<G> Matching<G>
where
    G: NodeCount,
{
    /// Returns `true` if the matching is perfect.
    ///
    /// A matching is
    /// [*perfect*](https://en.wikipedia.org/wiki/Matching_(graph_theory)#Definitions)
    /// if every node in the graph is incident to an edge from the matching.
    pub fn is_perfect(&self) -> bool {
        let n_nodes = self.graph.node_count();
        n_nodes % 2 == 0 && self.n_edges == n_nodes / 2
    }
}

trait WithDummy: NodeIndexable {
    fn dummy_idx(&self) -> usize;
    fn node_bound_with_dummy(&self) -> usize;
    /// Convert `i` to a node index, returns None for the dummy node
    fn try_from_index(&self, i: usize) -> Option<Self::NodeId>;
}

impl<G: NodeIndexable> WithDummy for G {
    fn dummy_idx(&self) -> usize {
        // Gabow numbers the vertices from 1 to n, and uses 0 as the dummy
        // vertex. Our vertex indices are zero-based and so we use the node
        // bound as the dummy node.
        self.node_bound()
    }

    fn node_bound_with_dummy(&self) -> usize {
        self.node_bound() + 1
    }

    fn try_from_index(&self, i: usize) -> Option<Self::NodeId> {
        if i != self.dummy_idx() {
            Some(self.from_index(i))
        } else {
            None
        }
    }
}

pub struct MatchedNodes<'a, G: GraphBase> {
    graph: &'a G,
    mate: &'a [Option<G::NodeId>],
    current: usize,
}

impl<G> Iterator for MatchedNodes<'_, G>
where
    G: NodeIndexable,
{
    type Item = G::NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current != self.mate.len() {
            let current = self.current;
            self.current += 1;

            if self.mate[current].is_some() {
                return Some(self.graph.from_index(current));
            }
        }

        None
    }
}

pub struct MatchedEdges<'a, G: GraphBase> {
    graph: &'a G,
    mate: &'a [Option<G::NodeId>],
    current: usize,
}

impl<G> Iterator for MatchedEdges<'_, G>
where
    G: NodeIndexable,
{
    type Item = (G::NodeId, G::NodeId);

    fn next(&mut self) -> Option<Self::Item> {
        while self.current != self.mate.len() {
            let current = self.current;
            self.current += 1;

            if let Some(mate) = self.mate[current] {
                // Check if the mate is a node after the current one. If not, then
                // do not report that edge since it has been already reported (the
                // graph is considered undirected).
                if self.graph.to_index(mate) > current {
                    let this = self.graph.from_index(current);
                    return Some((this, mate));
                }
            }
        }

        None
    }
}

/// \[Generic\] Compute a
/// [*matching*](https://en.wikipedia.org/wiki/Matching_(graph_theory)) using a
/// greedy heuristic.
///
/// The input graph is treated as if undirected. The underlying heuristic is
/// unspecified, but is guaranteed to be bounded by *O(|V| + |E|)*. No
/// guarantees about the output are given other than that it is a valid
/// matching.
///
/// If you require a maximum matching, use [`maximum_matching`][1] function
/// instead.
///
/// [1]: fn.maximum_matching.html
pub fn greedy_matching<G>(graph: G) -> Matching<G>
where
    G: Visitable + IntoNodeIdentifiers + NodeIndexable + IntoNeighbors,
    G::NodeId: Eq + Hash,
    G::EdgeId: Eq + Hash,
{
    let (mates, n_edges) = greedy_matching_inner(&graph);
    Matching::new(graph, mates, n_edges)
}

#[inline]
fn greedy_matching_inner<G>(graph: &G) -> (Vec<Option<G::NodeId>>, usize)
where
    G: Visitable + IntoNodeIdentifiers + NodeIndexable + IntoNeighbors,
{
    let mut mate = vec![None; graph.node_bound()];
    let mut n_edges = 0;
    let visited = &mut graph.visit_map();

    for start in graph.node_identifiers() {
        let mut last = Some(start);

        // Function non_backtracking_dfs does not expand the node if it has been
        // already visited.
        non_backtracking_dfs(graph, start, visited, |next| {
            // Alternate matched and unmatched edges.
            if let Some(pred) = last.take() {
                mate[graph.to_index(pred)] = Some(next);
                mate[graph.to_index(next)] = Some(pred);
                n_edges += 1;
            } else {
                last = Some(next);
            }
        });
    }

    (mate, n_edges)
}

fn non_backtracking_dfs<G, F>(graph: &G, source: G::NodeId, visited: &mut G::Map, mut visitor: F)
where
    G: Visitable + IntoNeighbors,
    F: FnMut(G::NodeId),
{
    if visited.visit(source) {
        for target in graph.neighbors(source) {
            if !visited.is_visited(&target) {
                visitor(target);
                non_backtracking_dfs(graph, target, visited, visitor);

                // Non-backtracking traversal, stop iterating over the
                // neighbors.
                break;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Label<G: GraphBase> {
    None,
    Start,
    // If node v is outer node, then label(v) = w is another outer node on path
    // from v to start u.
    Vertex(G::NodeId),
    // If node v is outer node, then label(v) = (r, s) are two outer vertices
    // (connected by an edge)
    Edge(G::EdgeId, [G::NodeId; 2]),
    // Flag is a special label used in searching for the join vertex of two
    // paths.
    Flag(G::EdgeId),
}

impl<G: GraphBase> Label<G> {
    fn is_outer(&self) -> bool {
        self != &Label::None
            && !match self {
                Label::Flag(_) => true,
                _ => false,
            }
    }

    fn is_inner(&self) -> bool {
        !self.is_outer()
    }

    fn to_vertex(&self) -> Option<G::NodeId> {
        match *self {
            Label::Vertex(v) => Some(v),
            _ => None,
        }
    }

    fn is_flagged(&self, edge: G::EdgeId) -> bool {
        match self {
            Label::Flag(flag) if flag == &edge => true,
            _ => false,
        }
    }
}

impl<G: GraphBase> Default for Label<G> {
    fn default() -> Self {
        Label::None
    }
}

impl<G: GraphBase> PartialEq for Label<G> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Label::None, Label::None) => true,
            (Label::Start, Label::Start) => true,
            (Label::Vertex(v1), Label::Vertex(v2)) => v1 == v2,
            (Label::Edge(e1, _), Label::Edge(e2, _)) => e1 == e2,
            (Label::Flag(e1), Label::Flag(e2)) => e1 == e2,
            _ => false,
        }
    }
}

/// \[Generic\] Compute the [*maximum
/// matching*](https://en.wikipedia.org/wiki/Matching_(graph_theory)) using
/// [Gabow's algorithm][1].
///
/// [1]: https://dl.acm.org/doi/10.1145/321941.321942
///
/// The input graph is treated as if undirected. The algorithm runs in
/// *O(|V|³)*. An algorithm with a better time complexity might be used in the
/// future.
///
/// **Panics** if `g.node_bound()` is `std::usize::MAX`.
///
/// # Examples
///
/// ```
/// use petgraph::prelude::*;
/// use petgraph::algo::maximum_matching;
///
/// // The example graph:
/// //
/// //    +-- b ---- d ---- f
/// //   /    |      |
/// //  a     |      |
/// //   \    |      |
/// //    +-- c ---- e
/// //
/// // Maximum matching: { (a, b), (c, e), (d, f) }
///
/// let mut graph: UnGraph<(), ()> = UnGraph::new_undirected();
/// let a = graph.add_node(());
/// let b = graph.add_node(());
/// let c = graph.add_node(());
/// let d = graph.add_node(());
/// let e = graph.add_node(());
/// let f = graph.add_node(());
/// graph.extend_with_edges(&[(a, b), (a, c), (b, c), (b, d), (c, e), (d, e), (d, f)]);
///
/// let matching = maximum_matching(&graph);
/// assert!(matching.contains_edge(a, b));
/// assert!(matching.contains_edge(c, e));
/// assert_eq!(matching.mate(d), Some(f));
/// assert_eq!(matching.mate(f), Some(d));
/// ```
pub fn maximum_matching<G>(graph: G) -> Matching<G>
where
    G: Visitable + NodeIndexable + IntoNodeIdentifiers + IntoEdges,
{
    // The dummy identifier needs an unused index
    assert_ne!(
        graph.node_bound(),
        std::usize::MAX,
        "The input graph capacity should be strictly less than std::usize::MAX."
    );

    // Greedy algorithm should create a fairly good initial matching. The hope
    // is that it speeds up the computation by doing les work in the complex
    // algorithm.
    let (mut mate, mut n_edges) = greedy_matching_inner(&graph);

    // Gabow's algorithm uses a dummy node in the mate array.
    mate.push(None);
    let len = graph.node_bound() + 1;
    debug_assert_eq!(mate.len(), len);

    let mut label: Vec<Label<G>> = vec![Label::None; len];
    let mut first_inner = vec![std::usize::MAX; len];
    let visited = &mut graph.visit_map();

    for start in 0..graph.node_bound() {
        if mate[start].is_some() {
            // The vertex is already matched. A start must be a free vertex.
            continue;
        }

        // Begin search from the node.
        label[start] = Label::Start;
        first_inner[start] = graph.dummy_idx();
        graph.reset_map(visited);

        // start is never a dummy index
        let start = graph.from_index(start);

        // Queue will contain outer vertices that should be processed next. The
        // start vertex is considered an outer vertex.
        let mut queue = VecDeque::new();
        queue.push_back(start);
        // Mark the start vertex so it is not processed repeatedly.
        visited.visit(start);

        'search: while let Some(outer_vertex) = queue.pop_front() {
            for edge in graph.edges(outer_vertex) {
                if edge.source() == edge.target() {
                    // Ignore self-loops.
                    continue;
                }

                let other_vertex = edge.target();
                let other_idx = graph.to_index(other_vertex);

                if mate[other_idx].is_none() && other_vertex != start {
                    // An augmenting path was found. Augment the matching. If
                    // `other` is actually the start node, then the augmentation
                    // must not be performed, because the start vertex would be
                    // incident to two edges, which violates the matching
                    // property.
                    mate[other_idx] = Some(outer_vertex);
                    augment_path(&graph, outer_vertex, other_vertex, &mut mate, &label);
                    n_edges += 1;

                    // The path is augmented, so the start is no longer free
                    // vertex. We need to begin with a new start.
                    break 'search;
                } else if label[other_idx].is_outer() {
                    // The `other` is an outer vertex (a label has been set to
                    // it). An odd cycle (blossom) was found. Assign this edge
                    // as a label to all inner vertices in paths P(outer) and
                    // P(other).
                    find_join(
                        &graph,
                        edge,
                        &mate,
                        &mut label,
                        &mut first_inner,
                        |labeled| {
                            if visited.visit(labeled) {
                                queue.push_back(labeled);
                            }
                        },
                    );
                } else {
                    let mate_vertex = mate[other_idx];
                    let mate_idx = mate_vertex.map_or(graph.dummy_idx(), |id| graph.to_index(id));

                    if label[mate_idx].is_inner() {
                        // Mate of `other` vertex is inner (no label has been
                        // set to it so far). But it actually is an outer vertex
                        // (it is on a path to the start vertex that begins with
                        // a matched edge, since it is a mate of `other`).
                        // Assign the label of this mate to the `outer` vertex,
                        // so the path for it can be reconstructed using `mate`
                        // and this label.
                        label[mate_idx] = Label::Vertex(outer_vertex);
                        first_inner[mate_idx] = other_idx;
                    }

                    // Add the vertex to the queue only if it's not the dummy and this is its first
                    // discovery.
                    if let Some(mate_vertex) = mate_vertex {
                        if visited.visit(mate_vertex) {
                            queue.push_back(mate_vertex);
                        }
                    }
                }
            }
        }

        // Reset the labels. All vertices are inner for the next search.
        for lbl in label.iter_mut() {
            *lbl = Label::None;
        }
    }

    // Discard the dummy node.
    mate.pop();

    Matching::new(graph, mate, n_edges)
}

fn find_join<G, F>(
    graph: &G,
    edge: G::EdgeRef,
    mate: &[Option<G::NodeId>],
    label: &mut [Label<G>],
    first_inner: &mut [usize],
    mut visitor: F,
) where
    G: IntoEdges + NodeIndexable + Visitable,
    F: FnMut(G::NodeId),
{
    // Simultaneously traverse the inner vertices on paths P(source) and
    // P(target) to find a join vertex - an inner vertex that is shared by these
    // paths.
    let source = graph.to_index(edge.source());
    let target = graph.to_index(edge.target());

    let mut left = first_inner[source];
    let mut right = first_inner[target];

    if left == right {
        // No vertices can be labeled, since both paths already refer to a
        // common vertex - the join.
        return;
    }

    // Flag the (first) inner vertices. This ensures that they are assigned the
    // join as their first inner vertex.
    let flag = Label::Flag(edge.id());
    label[left] = flag;
    label[right] = flag;

    // Find the join.
    let join = loop {
        // Swap the sides. Do not swap if the right side is already finished.
        if right != graph.dummy_idx() {
            std::mem::swap(&mut left, &mut right);
        }

        // Set left to the next inner vertex in P(source) or P(target).
        // The unwraps are safe because left is not the dummy node.
        let left_mate = graph.to_index(mate[left].unwrap());
        let next_inner = label[left_mate].to_vertex().unwrap();
        left = first_inner[graph.to_index(next_inner)];

        if !label[left].is_flagged(edge.id()) {
            // The inner vertex is not flagged yet, so flag it.
            label[left] = flag;
        } else {
            // The inner vertex is already flagged. It means that the other side
            // had to visit it already. Therefore it is the join vertex.
            break left;
        }
    };

    // Label all inner vertices on P(source) and P(target) with the found join.
    for endpoint in [source, target].iter().copied() {
        let mut inner = first_inner[endpoint];
        while inner != join {
            // Notify the caller about labeling a vertex.
            if let Some(ix) = graph.try_from_index(inner) {
                visitor(ix);
            }

            label[inner] = Label::Edge(edge.id(), [edge.source(), edge.target()]);
            first_inner[inner] = join;
            let inner_mate = graph.to_index(mate[inner].unwrap());
            let next_inner = label[inner_mate].to_vertex().unwrap();
            inner = first_inner[graph.to_index(next_inner)];
        }
    }

    for (vertex_idx, vertex_label) in label.iter().enumerate() {
        // To all outer vertices that are on paths P(source) and P(target) until
        // the join, se the join as their first inner vertex.
        if vertex_idx != graph.dummy_idx()
            && vertex_label.is_outer()
            && label[first_inner[vertex_idx]].is_outer()
        {
            first_inner[vertex_idx] = join;
        }
    }
}

fn augment_path<G>(
    graph: &G,
    outer: G::NodeId,
    other: G::NodeId,
    mate: &mut [Option<G::NodeId>],
    label: &[Label<G>],
) where
    G: NodeIndexable,
{
    let outer_idx = graph.to_index(outer);

    let temp = mate[outer_idx];
    let temp_idx = temp.map_or(graph.dummy_idx(), |id| graph.to_index(id));
    mate[outer_idx] = Some(other);

    if mate[temp_idx] != Some(outer) {
        // We are at the end of the path and so the entire path is completely
        // rematched/augmented.
    } else if let Label::Vertex(vertex) = label[outer_idx] {
        // The outer vertex has a vertex label which refers to another outer
        // vertex on the path. So we set this another outer node as the mate for
        // the previous mate of the outer node.
        mate[temp_idx] = Some(vertex);
        if let Some(temp) = temp {
            augment_path(graph, vertex, temp, mate, label);
        }
    } else if let Label::Edge(_, [source, target]) = label[outer_idx] {
        // The outer vertex has an edge label which refers to an edge in a
        // blossom. We need to augment both directions along the blossom.
        augment_path(graph, source, target, mate, label);
        augment_path(graph, target, source, mate, label);
    } else {
        panic!("Unexpected label when augmenting path");
    }
}
