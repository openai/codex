use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap as Map;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;
use std::ops::Index;

use fnv::FnvHasher;

mod fork;
mod impls;
mod meta;
mod range;
mod regex;
mod rope;

pub use self::fork::Fork;
pub use self::meta::Meta;
pub use self::range::Range;
pub use self::rope::Rope;

/// Disambiguation error during the attempt to merge two leaf
/// nodes with the same priority
#[derive(Debug)]
pub struct DisambiguationError(pub NodeId, pub NodeId);

pub struct Graph<Leaf> {
    /// Internal storage of all allocated nodes. Once a node is
    /// put here, it should never be mutated.
    nodes: Vec<Option<Node<Leaf>>>,
    /// When merging two nodes into a new node, we store the two
    /// entry keys and the result, so that we don't merge the same
    /// two nodes multiple times.
    ///
    /// Most of the time the entry we want to find will be the last
    /// one that has been inserted, so we can use a vec with reverse
    /// order search to get O(1) searches much faster than any *Map.
    merges: Map<Merge, NodeId>,
    /// Another map used for accounting. Before `.push`ing a new node
    /// onto the graph (inserts are exempt), we hash it and find if
    /// an identical(!) node has been created before.
    hashes: Map<u64, NodeId>,
    /// Instead of handling errors over return types, opt to collect
    /// them internally.
    errors: Vec<DisambiguationError>,
    /// Deferred merges. When when attempting to merge a node with an
    /// empty reserved slot, the merging operation is deferred until
    /// the reserved slot is populated. This is a stack that keeps track
    /// of all such deferred merges
    deferred: Vec<DeferredMerge>,
}

/// Trait to be implemented on `Leaf` nodes in order to disambiguate
/// between them.
pub trait Disambiguate {
    fn cmp(left: &Self, right: &Self) -> Ordering;
}

/// Id of a Node in the graph. `NodeId` can be referencing an empty
/// slot that is going to be populated later in time.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    fn get(self) -> usize {
        self.0.get() as usize
    }

    fn new(n: usize) -> NodeId {
        NodeId(NonZeroU32::new(n as u32).expect("Invalid NodeId"))
    }
}

/// Unique reserved `NodeId` that is guaranteed to point to an
/// empty allocated slot in the graph. It's safe to create multiple
/// `NodeId` copies of `ReservedId`, however API users should never
/// be able to clone a `ReservedId`, or create a new one from `NodeId`.
///
/// `ReservedId` is consumed once passed into `Graph::insert`.
#[derive(Debug)]
pub struct ReservedId(NodeId);

impl ReservedId {
    pub fn get(&self) -> NodeId {
        self.0
    }
}

/// Merge key used to lookup whether two nodes have been previously
/// mered, so we can avoid duplicating merges, potentially running into
/// loops that blow out the stack.
///
/// `Merge::new(a, b)` should always equal to `Merge::new(b, a)` to ensure
/// that node merges are symmetric.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Merge(NodeId, NodeId);

impl Merge {
    fn new(a: NodeId, b: NodeId) -> Self {
        if a < b {
            Merge(a, b)
        } else {
            Merge(b, a)
        }
    }
}

/// When attempting to merge two nodes, one of which was not yet created,
/// we can record such attempt, and execute the merge later on when the
/// `awaiting` has been `insert`ed into the graph.
#[derive(Debug)]
pub struct DeferredMerge {
    awaiting: NodeId,
    with: NodeId,
    into: ReservedId,
}

impl<Leaf> Graph<Leaf> {
    pub fn new() -> Self {
        Graph {
            // Start with an empty slot so we can start
            // counting NodeIds from 1 and use NonZero
            // optimizations
            nodes: vec![None],
            merges: Map::new(),
            hashes: Map::new(),
            errors: Vec::new(),
            deferred: Vec::new(),
        }
    }

    pub fn errors(&self) -> &[DisambiguationError] {
        &self.errors
    }

    fn next_id(&self) -> NodeId {
        NodeId::new(self.nodes.len())
    }

    /// Reserve an empty slot for a node on the graph and return an
    /// id for it. `ReservedId` cannot be cloned, and must be consumed
    /// by calling `insert` on the graph.
    pub fn reserve(&mut self) -> ReservedId {
        let id = self.next_id();

        self.nodes.push(None);

        ReservedId(id)
    }

    /// Insert a node at a given, previously reserved id. Returns the
    /// inserted `NodeId`.
    pub fn insert<N>(&mut self, reserved: ReservedId, node: N) -> NodeId
    where
        N: Into<Node<Leaf>>,
        Leaf: Disambiguate,
    {
        let id = reserved.get();

        self.nodes[id.get()] = Some(node.into());

        let mut awaiting = Vec::new();

        // Partition out all `DeferredMerge`s that can be completed
        // now that this `ReservedId` has a `Node` inserted into it.
        for idx in (0..self.deferred.len()).rev() {
            if self.deferred[idx].awaiting == id {
                awaiting.push(self.deferred.remove(idx));
            }
        }

        // Complete deferred merges. We've collected them from the back,
        // so we must iterate through them from the back as well to restore
        // proper order of merges in case there is some cascading going on.
        for DeferredMerge {
            awaiting,
            with,
            into,
        } in awaiting.into_iter().rev()
        {
            self.merge_unchecked(awaiting, with, into);
        }

        id
    }

    /// Push a node onto the graph and get an id to it. If an identical
    /// node has already been pushed on the graph, it will return the id
    /// of that node instead.
    pub fn push<B>(&mut self, node: B) -> NodeId
    where
        B: Into<Node<Leaf>>,
    {
        let node = node.into();

        if let Node::Leaf(_) = node {
            return self.push_unchecked(node);
        }

        let mut hasher = FnvHasher::default();
        node.hash(&mut hasher);

        let next_id = self.next_id();

        match self.hashes.entry(hasher.finish()) {
            Entry::Occupied(occupied) => {
                let id = *occupied.get();

                if self[id].eq(&node) {
                    return id;
                }
            }
            Entry::Vacant(vacant) => {
                vacant.insert(next_id);
            }
        }

        self.push_unchecked(node)
    }

    fn push_unchecked(&mut self, node: Node<Leaf>) -> NodeId {
        let id = self.next_id();

        self.nodes.push(Some(node));

        id
    }

    /// If nodes `a` and `b` have been already merged, return the
    /// `NodeId` of the node they have been merged into.
    fn find_merged(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let probe = Merge::new(a, b);

        self.merges.get(&probe).copied()
    }

    /// Mark that nodes `a` and `b` have been merged into `product`.
    ///
    /// This will also mark merging `a` and `product`, as well as
    /// `b` and `product` into `product`, since those are symmetric
    /// operations.
    ///
    /// This is necessary to break out asymmetric merge loops.
    fn set_merged(&mut self, a: NodeId, b: NodeId, product: NodeId) {
        self.merges.insert(Merge::new(a, b), product);
        self.merges.insert(Merge::new(a, product), product);
        self.merges.insert(Merge::new(b, product), product);
    }

    /// Merge the nodes at id `a` and `b`, returning a new id.
    pub fn merge(&mut self, a: NodeId, b: NodeId) -> NodeId
    where
        Leaf: Disambiguate,
    {
        if a == b {
            return a;
        }

        // If the id pair is already merged (or is being merged), just return the id
        if let Some(id) = self.find_merged(a, b) {
            return id;
        }

        match (self.get(a), self.get(b)) {
            (None, None) => {
                panic!(
                    "Merging two reserved nodes! This is a bug, please report it:\n\
                    \n\
                    https://github.com/maciejhirsz/logos/issues"
                );
            }
            (None, Some(_)) => {
                let reserved = self.reserve();
                let id = reserved.get();
                self.deferred.push(DeferredMerge {
                    awaiting: a,
                    with: b,
                    into: reserved,
                });
                self.set_merged(a, b, id);

                return id;
            }
            (Some(_), None) => {
                let reserved = self.reserve();
                let id = reserved.get();
                self.deferred.push(DeferredMerge {
                    awaiting: b,
                    with: a,
                    into: reserved,
                });
                self.set_merged(a, b, id);

                return id;
            }
            (Some(Node::Leaf(left)), Some(Node::Leaf(right))) => {
                return match Disambiguate::cmp(left, right) {
                    Ordering::Less => b,
                    Ordering::Greater => a,
                    Ordering::Equal => {
                        self.errors.push(DisambiguationError(a, b));

                        a
                    }
                };
            }
            _ => (),
        }

        // Reserve the id for the merge and save it. Since the graph can contain loops,
        // this prevents us from trying to merge the same id pair in a loop, blowing up
        // the stack.
        let reserved = self.reserve();
        self.set_merged(a, b, reserved.get());

        self.merge_unchecked(a, b, reserved)
    }

    /// Unchecked merge of `a` and `b`. This fn assumes that `a` and `b` are
    /// not pointing to empty slots.
    fn merge_unchecked(&mut self, a: NodeId, b: NodeId, reserved: ReservedId) -> NodeId
    where
        Leaf: Disambiguate,
    {
        let merged_rope = match (self.get(a), self.get(b)) {
            (Some(Node::Rope(rope)), _) => {
                let rope = rope.clone();

                self.merge_rope(rope, b)
            }
            (_, Some(Node::Rope(rope))) => {
                let rope = rope.clone();

                self.merge_rope(rope, a)
            }
            _ => None,
        };

        if let Some(rope) = merged_rope {
            return self.insert(reserved, rope);
        }

        let mut fork = self.fork_off(a);
        fork.merge(self.fork_off(b), self);

        let mut stack = vec![reserved.get()];

        // Flatten the fork
        while let Some(miss) = fork.miss {
            if stack.contains(&miss) {
                break;
            }
            stack.push(miss);

            let other = match self.get(miss) {
                Some(Node::Fork(other)) => other.clone(),
                Some(Node::Rope(other)) => other.clone().into_fork(self),
                _ => break,
            };
            match other.miss {
                Some(id) if self.get(id).is_none() => break,
                _ => (),
            }
            fork.miss = None;
            fork.merge(other, self);
        }

        self.insert(reserved, fork)
    }

    fn merge_rope(&mut self, rope: Rope, other: NodeId) -> Option<Rope>
    where
        Leaf: Disambiguate,
    {
        match self.get(other) {
            Some(Node::Fork(fork)) if rope.miss.is_none() => {
                // Count how many consecutive ranges in this rope would
                // branch into the fork that results in a loop.
                //
                // e.g.: for rope "foobar" and a looping fork [a-z]: 6
                let count = rope
                    .pattern
                    .iter()
                    .take_while(|range| fork.contains(**range) == Some(other))
                    .count();

                let mut rope = rope.split_at(count, self)?.miss_any(other);

                rope.then = self.merge(rope.then, other);

                Some(rope)
            }
            Some(Node::Rope(other)) => {
                let (prefix, miss) = rope.prefix(other)?;

                let (a, b) = (rope, other.clone());

                let a = a.remainder(prefix.len(), self);
                let b = b.remainder(prefix.len(), self);

                let rope = Rope::new(prefix, self.merge(a, b)).miss(miss);

                Some(rope)
            }
            Some(Node::Leaf(_)) | None => {
                if rope.miss.is_none() {
                    Some(rope.miss(other))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn fork_off(&mut self, id: NodeId) -> Fork
    where
        Leaf: Disambiguate,
    {
        match self.get(id) {
            Some(Node::Fork(fork)) => fork.clone(),
            Some(Node::Rope(rope)) => rope.clone().into_fork(self),
            Some(Node::Leaf(_)) | None => Fork::new().miss(id),
        }
    }

    pub fn nodes(&self) -> &[Option<Node<Leaf>>] {
        &self.nodes
    }

    /// Find all nodes that have no references and remove them.
    pub fn shake(&mut self, root: NodeId) {
        let mut filter = vec![false; self.nodes.len()];

        filter[root.get()] = true;

        self[root].shake(self, &mut filter);

        for (id, referenced) in filter.into_iter().enumerate() {
            if !referenced {
                self.nodes[id] = None;
            }
        }
    }

    pub fn get(&self, id: NodeId) -> Option<&Node<Leaf>> {
        self.nodes.get(id.get())?.as_ref()
    }
}

impl<Leaf> Index<NodeId> for Graph<Leaf> {
    type Output = Node<Leaf>;

    fn index(&self, id: NodeId) -> &Node<Leaf> {
        self.get(id).expect(
            "Indexing into an empty node. This is a bug, please report it at:\n\
            \n\
            https://github.com/maciejhirsz/logos/issues",
        )
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[cfg_attr(test, derive(PartialEq))]
pub enum Node<Leaf> {
    /// Fork node, can lead to more than one state
    Fork(Fork),
    /// Rope node, can lead to one state on match, one state on miss
    Rope(Rope),
    /// Leaf node, terminal state
    Leaf(Leaf),
}

impl<Leaf> Node<Leaf> {
    pub fn miss(&self) -> Option<NodeId> {
        match self {
            Node::Rope(rope) => rope.miss.first(),
            Node::Fork(fork) => fork.miss,
            Node::Leaf(_) => None,
        }
    }

    fn eq(&self, other: &Node<Leaf>) -> bool {
        match (self, other) {
            (Node::Fork(a), Node::Fork(b)) => a == b,
            (Node::Rope(a), Node::Rope(b)) => a == b,
            _ => false,
        }
    }

    fn shake(&self, graph: &Graph<Leaf>, filter: &mut [bool]) {
        match self {
            Node::Fork(fork) => fork.shake(graph, filter),
            Node::Rope(rope) => rope.shake(graph, filter),
            Node::Leaf(_) => (),
        }
    }

    pub fn unwrap_leaf(&self) -> &Leaf {
        match self {
            Node::Fork(_) => panic!("Internal Error: called unwrap_leaf on a fork"),
            Node::Rope(_) => panic!("Internal Error: called unwrap_leaf on a rope"),
            Node::Leaf(leaf) => leaf,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn leaf_stack_size() {
        use std::mem::size_of;

        const WORD: usize = size_of::<usize>();
        const NODE: usize = size_of::<Node<()>>();

        assert!(NODE <= 6 * WORD, "Size of Node<()> is {} bytes!", NODE);
    }

    #[test]
    fn create_a_loop() {
        let mut graph = Graph::new();

        let token = graph.push(Node::Leaf("IDENT"));
        let id = graph.reserve();
        let fork = Fork::new().branch('a'..='z', id.get()).miss(token);
        let root = graph.insert(id, fork);

        assert_eq!(graph[token], Node::Leaf("IDENT"));
        assert_eq!(graph[root], Fork::new().branch('a'..='z', root).miss(token),);
    }

    #[test]
    fn fork_off() {
        let mut graph = Graph::new();

        let leaf = graph.push(Node::Leaf("LEAF"));
        let rope = graph.push(Rope::new("rope", leaf));
        let fork = graph.push(Fork::new().branch(b'!', leaf));

        assert_eq!(graph.fork_off(leaf), Fork::new().miss(leaf));
        assert_eq!(
            graph.fork_off(rope),
            Fork::new().branch(b'r', NodeId::new(graph.nodes.len() - 1))
        );
        assert_eq!(graph.fork_off(fork), Fork::new().branch(b'!', leaf));
    }
}
