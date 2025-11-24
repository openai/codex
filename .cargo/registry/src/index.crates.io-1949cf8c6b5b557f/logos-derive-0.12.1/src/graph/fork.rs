use crate::graph::{Disambiguate, Graph, NodeId, Range};

#[derive(Clone)]
pub struct Fork {
    /// LUT matching byte -> node id
    lut: Box<[Option<NodeId>; 256]>,
    /// State to go to if no arms are matching
    pub miss: Option<NodeId>,
}

impl Fork {
    pub fn new() -> Self {
        Fork {
            lut: Box::new([None; 256]),
            miss: None,
        }
    }

    pub fn miss<M>(mut self, miss: M) -> Self
    where
        M: Into<Option<NodeId>>,
    {
        self.miss = miss.into();
        self
    }

    pub fn add_branch<R, T>(&mut self, range: R, then: NodeId, graph: &mut Graph<T>)
    where
        R: Into<Range>,
        T: Disambiguate,
    {
        for byte in range.into() {
            match &mut self.lut[byte as usize] {
                Some(other) if *other != then => {
                    *other = graph.merge(*other, then);
                }
                opt => *opt = Some(then),
            }
        }
    }

    // TODO: Add result with a printable error
    pub fn merge<T>(&mut self, other: Fork, graph: &mut Graph<T>)
    where
        T: Disambiguate,
    {
        self.miss = match (self.miss, other.miss) {
            (None, None) => None,
            (Some(id), None) | (None, Some(id)) => Some(id),
            (Some(a), Some(b)) => Some(graph.merge(a, b)),
        };

        for (left, right) in self.lut.iter_mut().zip(other.lut.iter()) {
            *left = match (*left, *right) {
                (None, None) => continue,
                (Some(id), None) | (None, Some(id)) => Some(id),
                (Some(a), Some(b)) => Some(graph.merge(a, b)),
            }
        }
    }

    pub fn branches(&self) -> ForkIter<'_> {
        ForkIter {
            offset: 0,
            lut: &*self.lut,
        }
    }

    /// Checks if all bytes in the `range` have a branch on this
    /// fork, and those branches are resolve to the same `NodeId`.
    pub fn contains<R>(&self, range: R) -> Option<NodeId>
    where
        R: Into<Range>,
    {
        let mut range = range.into();
        let byte = range.next()?;
        let first = self.lut[byte as usize]?;

        for byte in range {
            if first != self.lut[byte as usize]? {
                return None;
            }
        }

        Some(first)
    }

    pub fn branch<R>(mut self, range: R, then: NodeId) -> Self
    where
        R: Into<Range>,
    {
        for byte in range.into() {
            match &mut self.lut[byte as usize] {
                Some(other) if *other != then => {
                    panic!("Overlapping branches");
                }
                opt => *opt = Some(then),
            }
        }
        self
    }

    pub fn shake<T>(&self, graph: &Graph<T>, filter: &mut [bool]) {
        if let Some(id) = self.miss {
            if !filter[id.get()] {
                filter[id.get()] = true;
                graph[id].shake(graph, filter);
            }
        }

        for (_, id) in self.branches() {
            if !filter[id.get()] {
                filter[id.get()] = true;
                graph[id].shake(graph, filter);
            }
        }
    }
}

pub struct ForkIter<'a> {
    offset: usize,
    lut: &'a [Option<NodeId>; 256],
}

impl<'a> Iterator for ForkIter<'a> {
    type Item = (Range, NodeId);

    fn next(&mut self) -> Option<Self::Item> {
        // Consume empty slots
        self.offset += self.lut[self.offset..]
            .iter()
            .take_while(|next| next.is_none())
            .count();

        let then = self.lut.get(self.offset).copied().flatten()?;
        let start = self.offset;

        // Consume all slots with same NodeId target
        self.offset += self.lut[self.offset..]
            .iter()
            .take_while(|next| **next == Some(then))
            .count();

        Some((
            Range {
                start: start as u8,
                end: (self.offset - 1) as u8,
            },
            then,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;
    use pretty_assertions::assert_eq;

    #[test]
    fn fork_iter() {
        let mut buf = [None; 256];

        for byte in b'4'..=b'7' {
            buf[byte as usize] = Some(NodeId::new(1));
        }
        for byte in b'a'..=b'd' {
            buf[byte as usize] = Some(NodeId::new(2));
        }

        let iter = ForkIter {
            offset: 0,
            lut: &buf,
        };

        assert_eq!(
            &[
                (
                    Range {
                        start: b'4',
                        end: b'7'
                    },
                    NodeId::new(1)
                ),
                (
                    Range {
                        start: b'a',
                        end: b'd'
                    },
                    NodeId::new(2)
                ),
            ],
            &*iter.collect::<Vec<_>>(),
        );
    }

    #[test]
    fn merge_no_conflict() {
        let mut graph = Graph::new();

        let leaf1 = graph.push(Node::Leaf("FOO"));
        let leaf2 = graph.push(Node::Leaf("BAR"));

        let mut fork = Fork::new().branch(b'1', leaf1);

        fork.merge(Fork::new().branch(b'2', leaf2), &mut graph);

        assert_eq!(fork, Fork::new().branch(b'1', leaf1).branch(b'2', leaf2));
    }

    #[test]
    fn merge_miss_right() {
        let mut graph = Graph::new();

        let leaf1 = graph.push(Node::Leaf("FOO"));
        let leaf2 = graph.push(Node::Leaf("BAR"));

        let mut fork = Fork::new().branch(b'1', leaf1);

        fork.merge(Fork::new().miss(leaf2), &mut graph);

        assert_eq!(fork, Fork::new().branch(b'1', leaf1).miss(leaf2));
    }

    #[test]
    fn merge_miss_left() {
        let mut graph = Graph::new();

        let leaf1 = graph.push(Node::Leaf("FOO"));
        let leaf2 = graph.push(Node::Leaf("BAR"));

        let mut fork = Fork::new().miss(leaf1);

        fork.merge(Fork::new().branch(b'2', leaf2), &mut graph);

        assert_eq!(fork, Fork::new().branch(b'2', leaf2).miss(leaf1));
    }

    #[test]
    fn contains_byte() {
        let fork = Fork::new().branch('a'..='z', NodeId::new(42));

        assert_eq!(fork.contains(b't'), Some(NodeId::new(42)));
    }

    #[test]
    fn contains_range() {
        let fork = Fork::new()
            .branch('a'..='m', NodeId::new(42))
            .branch('n'..='z', NodeId::new(42));

        assert_eq!(fork.contains('i'..='r'), Some(NodeId::new(42)));
        assert_eq!(fork.contains('a'..='z'), Some(NodeId::new(42)));
    }

    #[test]
    fn contains_different_ranges() {
        let fork = Fork::new()
            .branch('a'..='m', NodeId::new(42))
            .branch('n'..='z', NodeId::new(47));

        assert_eq!(fork.contains('i'..='r'), None);
        assert_eq!(fork.contains('a'..='z'), None);
        assert_eq!(fork.contains('d'..='f'), Some(NodeId::new(42)));
        assert_eq!(fork.contains('n'..='p'), Some(NodeId::new(47)));
    }
}
