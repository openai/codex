use std::ops::Deref;

use crate::graph::{Disambiguate, Fork, Graph, NodeId, Range};

#[derive(PartialEq, Clone, Hash)]
pub struct Rope {
    pub pattern: Pattern,
    pub then: NodeId,
    pub miss: Miss,
}

#[derive(PartialEq, Clone, Hash)]
pub struct Pattern(pub Vec<Range>);

impl Deref for Pattern {
    type Target = [Range];

    fn deref(&self) -> &[Range] {
        &self.0
    }
}

/// Because Ropes could potentially fail a match mid-pattern,
/// a regular `Option` is not sufficient here.
#[derive(PartialEq, Clone, Copy, Hash)]
pub enum Miss {
    /// Same as Option::None, error on fail
    None,
    /// Jump to id if first byte does not match, fail on partial match
    First(NodeId),
    /// Jump to id on partial or empty match
    Any(NodeId),
}

impl Miss {
    pub fn is_none(&self) -> bool {
        matches!(self, Miss::None)
    }

    pub fn first(self) -> Option<NodeId> {
        match self {
            Miss::First(id) | Miss::Any(id) => Some(id),
            _ => None,
        }
    }

    pub fn take_first(&mut self) -> Option<NodeId> {
        match *self {
            Miss::First(id) => {
                *self = Miss::None;

                Some(id)
            }
            Miss::Any(id) => Some(id),
            Miss::None => None,
        }
    }
}

impl From<Option<NodeId>> for Miss {
    fn from(miss: Option<NodeId>) -> Self {
        match miss {
            Some(id) => Miss::First(id),
            None => Miss::None,
        }
    }
}

impl From<NodeId> for Miss {
    fn from(id: NodeId) -> Self {
        Miss::First(id)
    }
}

impl Rope {
    pub fn new<P>(pattern: P, then: NodeId) -> Self
    where
        P: Into<Pattern>,
    {
        Rope {
            pattern: pattern.into(),
            then,
            miss: Miss::None,
        }
    }

    pub fn miss<M>(mut self, miss: M) -> Self
    where
        M: Into<Miss>,
    {
        self.miss = miss.into();
        self
    }

    pub fn miss_any(mut self, miss: NodeId) -> Self {
        self.miss = Miss::Any(miss);
        self
    }

    pub fn into_fork<T>(mut self, graph: &mut Graph<T>) -> Fork
    where
        T: Disambiguate,
    {
        let first = self.pattern.0.remove(0);
        let miss = self.miss.take_first();

        // The new fork will lead to a new rope,
        // or the old target if no new rope was created
        let then = match self.pattern.len() {
            0 => self.then,
            _ => graph.push(self),
        };

        Fork::new().branch(first, then).miss(miss)
    }

    pub fn prefix(&self, other: &Self) -> Option<(Pattern, Miss)> {
        let count = self
            .pattern
            .iter()
            .zip(other.pattern.iter())
            .take_while(|(a, b)| a == b)
            .count();

        let pattern = match count {
            0 => return None,
            n => self.pattern[..n].into(),
        };
        let miss = match (self.miss, other.miss) {
            (Miss::None, miss) => miss,
            (miss, Miss::None) => miss,
            _ => return None,
        };

        Some((pattern, miss))
    }

    pub fn split_at<T>(mut self, at: usize, graph: &mut Graph<T>) -> Option<Rope>
    where
        T: Disambiguate,
    {
        match at {
            0 => return None,
            n if n == self.pattern.len() => return Some(self),
            _ => (),
        }

        let (this, next) = self.pattern.split_at(at);

        let next_miss = match self.miss {
            Miss::Any(_) => self.miss,
            _ => Miss::None,
        };

        let next = graph.push(Rope {
            pattern: next.into(),
            miss: next_miss,
            then: self.then,
        });

        self.pattern = this.into();
        self.then = next;

        Some(self)
    }

    pub fn remainder<T>(mut self, at: usize, graph: &mut Graph<T>) -> NodeId
    where
        T: Disambiguate,
    {
        self.pattern = self.pattern[at..].into();

        match self.pattern.len() {
            0 => self.then,
            _ => graph.push(self),
        }
    }

    pub fn shake<T>(&self, graph: &Graph<T>, filter: &mut [bool]) {
        if let Some(id) = self.miss.first() {
            if !filter[id.get()] {
                filter[id.get()] = true;
                graph[id].shake(graph, filter);
            }
        }

        if !filter[self.then.get()] {
            filter[self.then.get()] = true;
            graph[self.then].shake(graph, filter);
        }
    }
}

impl Pattern {
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(self.len());

        for range in self.iter() {
            out.push(range.as_byte()?);
        }

        Some(out)
    }
}

impl<T> From<&[T]> for Pattern
where
    T: Into<Range> + Copy,
{
    fn from(slice: &[T]) -> Self {
        Pattern(slice.iter().copied().map(Into::into).collect())
    }
}

impl<T> From<Vec<T>> for Pattern
where
    T: Into<Range>,
{
    fn from(vec: Vec<T>) -> Self {
        Pattern(vec.into_iter().map(Into::into).collect())
    }
}

impl From<&str> for Pattern {
    fn from(slice: &str) -> Self {
        slice.as_bytes().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;
    use pretty_assertions::assert_eq;

    #[test]
    fn into_fork() {
        let mut graph = Graph::new();

        let leaf = graph.push(Node::Leaf("LEAF"));
        let rope = Rope::new("foobar", leaf);

        let fork = rope.into_fork(&mut graph);

        assert_eq!(leaf, NodeId::new(1));
        assert_eq!(fork, Fork::new().branch(b'f', NodeId::new(2)));
        assert_eq!(graph[NodeId::new(2)], Rope::new("oobar", leaf));
    }

    #[test]
    fn into_fork_one_byte() {
        let mut graph = Graph::new();

        let leaf = graph.push(Node::Leaf("LEAF"));
        let rope = Rope::new("!", leaf);

        let fork = rope.into_fork(&mut graph);

        assert_eq!(leaf, NodeId::new(1));
        assert_eq!(fork, Fork::new().branch(b'!', NodeId::new(1)));
    }

    #[test]
    fn into_fork_miss_any() {
        let mut graph = Graph::new();

        let leaf = graph.push(Node::Leaf("LEAF"));
        let rope = Rope::new("42", leaf).miss_any(NodeId::new(42));

        let fork = rope.into_fork(&mut graph);

        assert_eq!(leaf, NodeId::new(1));
        assert_eq!(
            fork,
            Fork::new()
                .branch(b'4', NodeId::new(2))
                .miss(NodeId::new(42))
        );
        assert_eq!(
            graph[NodeId::new(2)],
            Rope::new("2", leaf).miss_any(NodeId::new(42))
        );
    }

    #[test]
    fn into_fork_miss_first() {
        let mut graph = Graph::new();

        let leaf = graph.push(Node::Leaf("LEAF"));
        let rope = Rope::new("42", leaf).miss(Miss::First(NodeId::new(42)));

        let fork = rope.into_fork(&mut graph);

        assert_eq!(leaf, NodeId::new(1));
        assert_eq!(
            fork,
            Fork::new()
                .branch(b'4', NodeId::new(2))
                .miss(NodeId::new(42))
        );
        assert_eq!(graph[NodeId::new(2)], Rope::new("2", leaf));
    }

    #[test]
    fn split_at() {
        let mut graph = Graph::new();

        let leaf = graph.push(Node::Leaf("LEAF"));
        let rope = Rope::new("foobar", leaf);

        assert_eq!(rope.clone().split_at(6, &mut graph).unwrap(), rope);

        let split = rope.split_at(3, &mut graph).unwrap();
        let expected_id = NodeId::new(leaf.get() + 1);

        assert_eq!(split, Rope::new("foo", expected_id));
        assert_eq!(graph[expected_id], Rope::new("bar", leaf));
    }

    #[test]
    fn pattern_to_bytes() {
        let pat = Pattern::from("foobar");

        assert_eq!(pat.to_bytes().unwrap(), b"foobar");

        let ranges = Pattern::from(vec![0..=0, 42..=42, b'{'..=b'}']);

        assert_eq!(ranges.to_bytes(), None);
    }
}
