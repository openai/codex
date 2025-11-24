use std::{
    collections::{HashMap, VecDeque},
    ops::{Index, IndexMut},
};

use crate::{
    graph::{GraphIndex, NodeIndex},
    visit::{EdgeRef, GraphProp, IntoEdgeReferences},
    Directed,
};

use self::linked_list::{LinkedList, LinkedListEntry};

/// \[Generic\] Finds a [feedback arc set]: a set of edges in the given directed graph, which when
/// removed, make the graph acyclic.
///
/// Uses a [greedy heuristic algorithm] to select a small number of edges, but does not necessarily
/// find the minimum feedback arc set. Time complexity is roughly **O(|E|)** for an input graph with
/// edges **E**.
///
/// Does not consider edge/node weights when selecting edges for the feedback arc set.
///
/// Loops (edges to and from the same node) are always included in the returned set.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "stable_graph")] {
/// use petgraph::{
///     algo::{greedy_feedback_arc_set, is_cyclic_directed},
///     graph::EdgeIndex,
///     stable_graph::StableGraph,
///     visit::EdgeRef,
/// };
///
/// let mut g: StableGraph<(), ()> = StableGraph::from_edges(&[
///     (0, 1),
///     (1, 2),
///     (2, 3),
///     (3, 4),
///     (4, 5),
///     (5, 0),
///     (4, 1),
///     (1, 3),
/// ]);
///
/// assert!(is_cyclic_directed(&g));
///
/// let fas: Vec<EdgeIndex> = greedy_feedback_arc_set(&g).map(|e| e.id()).collect();
///
/// // Remove edges in feedback arc set from original graph
/// for edge_id in fas {
///     g.remove_edge(edge_id);
/// }
///
/// assert!(!is_cyclic_directed(&g));
/// # }
/// ```
///
/// [feedback arc set]: https://en.wikipedia.org/wiki/Feedback_arc_set
/// [greedy heuristic algorithm]: https://doi.org/10.1016/0020-0190(93)90079-O
pub fn greedy_feedback_arc_set<G>(g: G) -> impl Iterator<Item = G::EdgeRef>
where
    G: IntoEdgeReferences + GraphProp<EdgeType = Directed>,
    G::NodeId: GraphIndex,
    G: crate::visit::NodeCount,
{
    let node_seq = good_node_sequence(g.edge_references().map(|e| {
        (
            NodeIndex::new(e.source().index()),
            NodeIndex::new(e.target().index()),
        )
    }));

    g.edge_references()
        .filter(move |e| node_seq[&e.source().index()] >= node_seq[&e.target().index()])
}

fn good_node_sequence(
    edge_refs: impl Iterator<Item = (NodeIndex<usize>, NodeIndex<usize>)>,
) -> HashMap<usize, usize> {
    let mut nodes = FasNodeContainer { nodes: Vec::new() };
    let mut buckets = Buckets {
        sinks_or_isolated: NodeLinkedList::new(),
        sources: NodeLinkedList::new(),
        bidirectional_pve_dd: Vec::new(),
        bidirectional_nve_dd: Vec::new(),
    };
    // Lookup of node indices from input graph to indices into `nodes`
    let mut graph_ix_lookup = HashMap::new();

    // Build node entries
    for (from_g_ix, to_g_ix) in edge_refs {
        let mut fas_node_entry = |g_ix: NodeIndex<usize>| -> FasNodeIndex {
            match graph_ix_lookup.get(&g_ix) {
                Some(fas_ix) => *fas_ix,
                None => {
                    let fas_ix = FasNodeIndex(nodes.nodes.len());

                    nodes.nodes.push(LinkedListEntry::new(FasNode {
                        graph_ix: g_ix,
                        out_edges: Vec::new(),
                        in_edges: Vec::new(),
                        out_degree: 0,
                        in_degree: 0,
                    }));

                    graph_ix_lookup.insert(g_ix, fas_ix);

                    fas_ix
                }
            }
        };

        let from_fas_ix = fas_node_entry(from_g_ix);
        let to_fas_ix = fas_node_entry(to_g_ix);

        nodes[from_fas_ix].data().out_edges.push(to_fas_ix);
        nodes[to_fas_ix].data().in_edges.push(from_fas_ix);
    }

    // Set initial in/out-degrees
    for entry in nodes.nodes.iter_mut() {
        let node = entry.data();
        node.out_degree = node.out_edges.len();
        node.in_degree = node.in_edges.len();
    }

    // Add nodes to initial lists
    for i in 0..nodes.nodes.len() {
        let fas_ix = FasNodeIndex(i);
        buckets
            .suitable_bucket(fas_ix, &mut nodes)
            .push_front(fas_ix, &mut nodes);
    }

    let mut s_1 = VecDeque::new();
    let mut s_2 = VecDeque::new();

    loop {
        let mut some_moved = false;

        while let Some(sink_fas_ix) = buckets.sinks_or_isolated.pop(&mut nodes) {
            some_moved = true;
            buckets.update_neighbour_node_buckets(sink_fas_ix, &mut nodes);
            s_2.push_front(nodes[sink_fas_ix].data().graph_ix);
        }

        while let Some(source_fas_ix) = buckets.sources.pop(&mut nodes) {
            some_moved = true;
            buckets.update_neighbour_node_buckets(source_fas_ix, &mut nodes);
            s_1.push_back(nodes[source_fas_ix].data().graph_ix);
        }

        if let Some(list) = buckets
            .bidirectional_pve_dd
            .iter_mut()
            .rev()
            .chain(buckets.bidirectional_nve_dd.iter_mut())
            .find(|b| b.start.is_some())
        {
            let highest_dd_fas_ix = list.pop(&mut nodes).unwrap();
            some_moved = true;
            buckets.update_neighbour_node_buckets(highest_dd_fas_ix, &mut nodes);
            s_1.push_back(nodes[highest_dd_fas_ix].data().graph_ix);

            Buckets::trim_bucket_list(&mut buckets.bidirectional_pve_dd);
            Buckets::trim_bucket_list(&mut buckets.bidirectional_nve_dd);
        }

        if !some_moved {
            break;
        }
    }

    s_1.into_iter()
        .chain(s_2)
        .enumerate()
        .map(|(seq_order, node_index)| (node_index.index(), seq_order))
        .collect()
}

type NodeLinkedList = LinkedList<FasNode, FasNodeContainer, FasNodeIndex>;

#[derive(Debug)]
struct FasNodeContainer {
    nodes: Vec<LinkedListEntry<FasNode, FasNodeIndex>>,
}

impl Index<FasNodeIndex> for FasNodeContainer {
    type Output = LinkedListEntry<FasNode, FasNodeIndex>;

    fn index(&self, index: FasNodeIndex) -> &Self::Output {
        &self.nodes[index.0]
    }
}

impl IndexMut<FasNodeIndex> for FasNodeContainer {
    fn index_mut(&mut self, index: FasNodeIndex) -> &mut Self::Output {
        &mut self.nodes[index.0]
    }
}

#[derive(Debug)]
struct Buckets {
    sinks_or_isolated: NodeLinkedList,
    sources: NodeLinkedList,
    /// Bidirectional nodes with positive-or-0 delta degree
    bidirectional_pve_dd: Vec<NodeLinkedList>,
    /// Bidirectional nodes with negative delta degree (index 0 is -1 dd, 1 is -2 etc)
    bidirectional_nve_dd: Vec<NodeLinkedList>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct FasNodeIndex(usize);

/// Represents a node from the input graph, tracking its current delta degree
#[derive(Debug)]
struct FasNode {
    /// Node index in input graph.
    graph_ix: NodeIndex<usize>,

    /// All outward edges from this node (not removed during processing)
    out_edges: Vec<FasNodeIndex>,

    /// All inward edges from this node (not removed during processing)
    in_edges: Vec<FasNodeIndex>,

    /// Current out-degree of this node (decremented during processing as connected nodes are
    /// removed)
    out_degree: usize,

    /// Current in-degree of this node (decremented during processing as connected nodes are
    /// removed)
    in_degree: usize,
}

impl Buckets {
    fn suitable_bucket(
        &mut self,
        ix: FasNodeIndex,
        nodes: &mut FasNodeContainer,
    ) -> &mut NodeLinkedList {
        let node = nodes[ix].data();

        if node.out_degree == 0 {
            &mut self.sinks_or_isolated
        } else if node.in_degree == 0 {
            &mut self.sources
        } else {
            let delta_degree = node.out_degree as isize - node.in_degree as isize;

            if delta_degree >= 0 {
                let bucket_ix = delta_degree as usize;

                if self.bidirectional_pve_dd.len() <= bucket_ix {
                    self.bidirectional_pve_dd
                        .resize_with(bucket_ix + 1, NodeLinkedList::new);
                }

                &mut self.bidirectional_pve_dd[bucket_ix]
            } else {
                let bucket_ix = (-delta_degree - 1) as usize;

                if self.bidirectional_nve_dd.len() <= bucket_ix {
                    self.bidirectional_nve_dd
                        .resize_with(bucket_ix + 1, NodeLinkedList::new);
                }

                &mut self.bidirectional_nve_dd[bucket_ix]
            }
        }
    }

    fn update_neighbour_node_buckets(&mut self, ix: FasNodeIndex, nodes: &mut FasNodeContainer) {
        for i in 0..nodes[ix].data().out_edges.len() {
            let out_ix = nodes[ix].data().out_edges[i];

            if out_ix == ix {
                continue;
            }

            // Ignore nodes which have already been moved to the good sequence
            if !nodes[out_ix].is_in_list() {
                continue;
            }

            self.suitable_bucket(out_ix, nodes).remove(out_ix, nodes);

            // Other node has lost an in-edge; reduce in-degree by 1
            nodes[out_ix].data().in_degree -= 1;

            self.suitable_bucket(out_ix, nodes)
                .push_front(out_ix, nodes);
        }

        for i in 0..nodes[ix].data().in_edges.len() {
            let in_ix = nodes[ix].data().in_edges[i];

            if in_ix == ix {
                continue;
            }

            // Ignore nodes which have already been moved to the good sequence
            if !nodes[in_ix].is_in_list() {
                continue;
            }

            self.suitable_bucket(in_ix, nodes).remove(in_ix, nodes);

            // Other node has lost an out-edge; reduce out-degree by 1
            nodes[in_ix].data().out_degree -= 1;

            self.suitable_bucket(in_ix, nodes).push_front(in_ix, nodes);
        }
    }

    fn trim_bucket_list(list: &mut Vec<NodeLinkedList>) {
        let trunc_len = if let Some(highest_populated_index) =
            (0..list.len()).rev().find(|i| list[*i].start.is_some())
        {
            highest_populated_index + 1
        } else {
            0
        };

        list.truncate(trunc_len);
    }
}

mod linked_list {
    use std::{marker::PhantomData, ops::IndexMut};

    #[derive(PartialEq, Debug)]
    pub struct LinkedList<Data, Container, Ix> {
        pub start: Option<Ix>,
        marker: PhantomData<(Data, Container)>,
    }

    #[derive(Debug)]
    pub struct LinkedListEntry<Data, Ix> {
        pos: Option<LinkedListPosition<Ix>>,
        data: Data,
    }

    #[derive(Debug)]
    struct LinkedListPosition<Ix> {
        prev: Option<Ix>,
        next: Option<Ix>,
    }

    impl<Data, Ix> LinkedListEntry<Data, Ix> {
        pub fn new(data: Data) -> Self {
            LinkedListEntry { pos: None, data }
        }

        pub fn data(&mut self) -> &mut Data {
            &mut self.data
        }

        pub fn is_in_list(&mut self) -> bool {
            self.pos.is_some()
        }

        fn pos_mut(&mut self) -> &mut LinkedListPosition<Ix> {
            self.pos
                .as_mut()
                .expect("expected linked list entry to have populated position")
        }
    }

    impl<Data, Container, Ix> LinkedList<Data, Container, Ix>
    where
        Container: IndexMut<Ix, Output = LinkedListEntry<Data, Ix>>,
        Ix: PartialEq + Copy,
    {
        pub fn new() -> Self {
            LinkedList {
                start: None,
                marker: PhantomData,
            }
        }

        pub fn push_front(&mut self, push_ix: Ix, container: &mut Container) {
            if let Some(start_ix) = self.start {
                let entry = &mut container[start_ix];
                entry.pos_mut().prev = Some(push_ix);
            }

            let push_entry = &mut container[push_ix];
            push_entry.pos = Some(LinkedListPosition {
                next: self.start,
                prev: None,
            });

            self.start = Some(push_ix);
        }

        pub fn pop(&mut self, container: &mut Container) -> Option<Ix> {
            if let Some(remove_ix) = self.start {
                self.remove(remove_ix, container);
                Some(remove_ix)
            } else {
                None
            }
        }

        /// `remove_ix` **must** be a member of the list headed by `self`
        pub fn remove(&mut self, remove_ix: Ix, container: &mut Container) {
            debug_assert!(
                self.to_vec(container).contains(&remove_ix),
                "node to remove should be member of current linked list"
            );

            let remove_entry = &mut container[remove_ix];
            let ll_entry = remove_entry.pos.take().unwrap();

            if let Some(prev_ix) = ll_entry.prev {
                let prev_node = &mut container[prev_ix];
                prev_node.pos_mut().next = ll_entry.next;
            }

            if let Some(next_ix) = ll_entry.next {
                let next_node = &mut container[next_ix];
                next_node.pos_mut().prev = ll_entry.prev;
            }

            // If the removed node was head of the list
            if self.start == Some(remove_ix) {
                self.start = ll_entry.next;
            }
        }

        /// For debug purposes
        fn to_vec(&self, container: &mut Container) -> Vec<Ix> {
            let mut ixs = Vec::new();

            let mut node_ix = self.start;

            while let Some(n_ix) = node_ix {
                ixs.push(n_ix);

                node_ix = container[n_ix].pos_mut().next;
            }

            ixs
        }
    }
}
