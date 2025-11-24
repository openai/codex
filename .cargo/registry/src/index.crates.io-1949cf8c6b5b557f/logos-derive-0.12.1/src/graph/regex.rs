use std::fmt::Debug;

use regex_syntax::utf8::Utf8Sequences;

use crate::graph::{Disambiguate, Fork, Graph, Node, NodeId, Range, ReservedId, Rope};
use crate::mir::{Class, ClassUnicode, Literal, Mir};

impl<Leaf: Disambiguate + Debug> Graph<Leaf> {
    pub fn regex(&mut self, mir: Mir, then: NodeId) -> NodeId {
        self.parse_mir(mir, then, None, None)
    }

    fn parse_mir(
        &mut self,
        mir: Mir,
        then: NodeId,
        miss: Option<NodeId>,
        reserved: Option<ReservedId>,
    ) -> NodeId {
        match mir {
            Mir::Empty => then,
            Mir::Loop(mir) => {
                let miss = match miss {
                    Some(id) => self.merge(id, then),
                    None => then,
                };
                let this = match reserved {
                    Some(rid) => rid,
                    None => self.reserve(),
                };

                self.parse_mir(*mir, this.get(), Some(miss), Some(this))
            }
            Mir::Maybe(mir) => {
                let miss = match miss {
                    Some(id) => self.merge(id, then),
                    None => then,
                };

                self.parse_mir(*mir, then, Some(miss), reserved)
            }
            Mir::Alternation(alternation) => {
                let mut fork = Fork::new().miss(miss);

                for mir in alternation {
                    let id = self.parse_mir(mir, then, None, None);
                    let alt = self.fork_off(id);

                    fork.merge(alt, self);
                }

                self.insert_or_push(reserved, fork)
            }
            Mir::Literal(literal) => {
                let pattern = match literal {
                    Literal::Unicode(unicode) => {
                        unicode.encode_utf8(&mut [0; 4]).as_bytes().to_vec()
                    }
                    Literal::Byte(byte) => [byte].to_vec(),
                };

                self.insert_or_push(reserved, Rope::new(pattern, then).miss(miss))
            }
            Mir::Concat(mut concat) => {
                // We'll be writing from the back, so need to allocate enough
                // space here. Worst case scenario is all unicode codepoints
                // producing 4 byte utf8 sequences
                let mut ropebuf = vec![Range::from(0); concat.len() * 4];
                let mut cur = ropebuf.len();
                let mut end = ropebuf.len();
                let mut then = then;

                let mut handle_bytes = |graph: &mut Self, mir, then: &mut NodeId| match mir {
                    Mir::Literal(Literal::Unicode(u)) => {
                        cur -= u.len_utf8();
                        for (i, byte) in u.encode_utf8(&mut [0; 4]).bytes().enumerate() {
                            ropebuf[cur + i] = byte.into();
                        }
                        None
                    }
                    Mir::Literal(Literal::Byte(byte)) => {
                        cur -= 1;
                        ropebuf[cur] = byte.into();
                        None
                    }
                    Mir::Class(Class::Unicode(class)) if is_one_ascii(&class) => {
                        cur -= 1;
                        ropebuf[cur] = class.ranges()[0].into();
                        None
                    }
                    Mir::Class(Class::Bytes(class)) if class.ranges().len() == 1 => {
                        cur -= 1;
                        ropebuf[cur] = class.ranges()[0].into();
                        None
                    }
                    mir => {
                        if end > cur {
                            let rope = Rope::new(&ropebuf[cur..end], *then);

                            *then = graph.push(rope);
                            end = cur;
                        }

                        Some(mir)
                    }
                };

                for mir in concat.drain(1..).rev() {
                    if let Some(mir) = handle_bytes(self, mir, &mut then) {
                        then = self.parse_mir(mir, then, None, None);
                    }
                }

                match handle_bytes(self, concat.remove(0), &mut then) {
                    None => {
                        let rope = Rope::new(&ropebuf[cur..end], then).miss(miss);

                        self.insert_or_push(reserved, rope)
                    }
                    Some(mir) => self.parse_mir(mir, then, miss, reserved),
                }
            }
            Mir::Class(Class::Unicode(class)) if !is_ascii(&class) => {
                let mut ropes = class
                    .iter()
                    .flat_map(|range| Utf8Sequences::new(range.start(), range.end()))
                    .map(|sequence| Rope::new(sequence.as_slice(), then))
                    .collect::<Vec<_>>();

                if ropes.len() == 1 {
                    let rope = ropes.remove(0);

                    return self.insert_or_push(reserved, rope.miss(miss));
                }

                let mut root = Fork::new().miss(miss);

                for rope in ropes {
                    let fork = rope.into_fork(self);
                    root.merge(fork, self);
                }

                self.insert_or_push(reserved, root)
            }
            Mir::Class(class) => {
                let mut fork = Fork::new().miss(miss);

                let class: Vec<Range> = match class {
                    Class::Unicode(u) => u.iter().copied().map(Into::into).collect(),
                    Class::Bytes(b) => b.iter().copied().map(Into::into).collect(),
                };

                for range in class {
                    fork.add_branch(range, then, self);
                }

                self.insert_or_push(reserved, fork)
            }
        }
    }

    fn insert_or_push<N>(&mut self, id: Option<ReservedId>, node: N) -> NodeId
    where
        N: Into<Node<Leaf>>,
    {
        match id {
            Some(id) => self.insert(id, node),
            None => self.push(node),
        }
    }
}

fn is_ascii(class: &ClassUnicode) -> bool {
    class.iter().all(|range| {
        let start = range.start() as u32;
        let end = range.end() as u32;

        start < 128 && (end < 128 || end == 0x0010_FFFF)
    })
}

fn is_one_ascii(class: &ClassUnicode) -> bool {
    if class.ranges().len() != 1 {
        return false;
    }

    let range = &class.ranges()[0];
    let start = range.start() as u32;
    let end = range.end() as u32;

    start < 128 && (end < 128 || end == 0x0010_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;
    use pretty_assertions::assert_eq;

    #[test]
    fn rope() {
        let mut graph = Graph::new();

        let mir = Mir::utf8("foobar").unwrap();

        assert_eq!(mir.priority(), 12);

        let leaf = graph.push(Node::Leaf("LEAF"));
        let id = graph.regex(mir, leaf);

        assert_eq!(graph[id], Node::Rope(Rope::new("foobar", leaf)),)
    }

    #[test]
    fn alternation() {
        let mut graph = Graph::new();

        let mir = Mir::utf8("a|b").unwrap();

        assert_eq!(mir.priority(), 2);

        let leaf = graph.push(Node::Leaf("LEAF"));
        let id = graph.regex(mir, leaf);

        assert_eq!(
            graph[id],
            Node::Fork(Fork::new().branch(b'a', leaf).branch(b'b', leaf)),
        );
    }

    #[test]
    fn repeat() {
        let mut graph = Graph::new();

        let mir = Mir::utf8("[a-z]*").unwrap();

        assert_eq!(mir.priority(), 0);

        let leaf = graph.push(Node::Leaf("LEAF"));
        let id = graph.regex(mir, leaf);

        assert_eq!(
            graph[id],
            Node::Fork(
                Fork::new()
                    .branch('a'..='z', id) // goto self == loop
                    .miss(leaf)
            ),
        );
    }

    #[test]
    fn maybe() {
        let mut graph = Graph::new();

        let mir = Mir::utf8("[a-z]?").unwrap();

        assert_eq!(mir.priority(), 0);

        let leaf = graph.push(Node::Leaf("LEAF"));
        let id = graph.regex(mir, leaf);

        assert_eq!(
            graph[id],
            Node::Fork(Fork::new().branch('a'..='z', leaf).miss(leaf)),
        );
    }
}
