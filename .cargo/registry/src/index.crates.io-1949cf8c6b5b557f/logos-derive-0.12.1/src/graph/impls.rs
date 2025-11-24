use std::fmt::{self, Debug, Display};
use std::hash::{Hash, Hasher};

use crate::graph::{Fork, Graph, Node, NodeId, Range, Rope};

impl<T> From<Fork> for Node<T> {
    fn from(fork: Fork) -> Self {
        Node::Fork(fork)
    }
}
impl<T> From<Rope> for Node<T> {
    fn from(rope: Rope) -> Self {
        Node::Rope(rope)
    }
}

fn is_ascii(byte: u8) -> bool {
    (byte >= 0x20) & (byte < 0x7F)
}

impl Hash for Fork {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for branch in self.branches() {
            branch.hash(state);
        }
        self.miss.hash(state);
    }
}

impl<T> Hash for Node<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Node::Rope(rope) => {
                b"ROPE".hash(state);
                rope.hash(state);
            }
            Node::Fork(fork) => {
                b"FORK".hash(state);
                fork.hash(state);
            }
            Node::Leaf(_) => b"LEAF".hash(state),
        }
    }
}

impl Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

/// We don't need debug impls in release builds
// #[cfg(test)]
mod debug {
    use super::*;
    use crate::graph::rope::Miss;
    use crate::graph::Disambiguate;
    use std::cmp::{Ord, Ordering};

    impl Disambiguate for &str {
        fn cmp(left: &&str, right: &&str) -> Ordering {
            Ord::cmp(left, right)
        }
    }

    impl Debug for Range {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let Range { start, end } = *self;

            if start != end || !is_ascii(start) {
                f.write_str("[")?;
            }
            match is_ascii(start) {
                true => write!(f, "{}", start as char),
                false => write!(f, "{:02X}", start),
            }?;
            if start != end {
                match is_ascii(end) {
                    true => write!(f, "-{}]", end as char),
                    false => write!(f, "-{:02X}]", end),
                }?;
            } else if !is_ascii(start) {
                f.write_str("]")?;
            }
            Ok(())
        }
    }

    impl Display for Range {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            <Range as Debug>::fmt(self, f)
        }
    }

    impl<T: Debug> Debug for Graph<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let entries = self
                .nodes()
                .iter()
                .enumerate()
                .filter_map(|(i, n)| n.as_ref().map(|n| (i, n)));

            f.debug_map().entries(entries).finish()
        }
    }

    struct Arm<T, U>(T, U);

    impl<T, U> Debug for Arm<T, U>
    where
        T: Display,
        U: Display,
    {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{} ⇒ {}", self.0, self.1)
        }
    }

    impl Debug for Fork {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let mut list = f.debug_set();

            for (range, then) in self.branches() {
                list.entry(&Arm(range, then));
            }
            if let Some(id) = self.miss {
                list.entry(&Arm('_', id));
            }

            list.finish()
        }
    }

    impl Display for Miss {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Miss::First(id) => Display::fmt(id, f),
                Miss::Any(id) => write!(f, "{}*", id),
                Miss::None => f.write_str("n/a"),
            }
        }
    }

    impl Debug for Rope {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            use std::fmt::Write;

            let mut rope = String::with_capacity(self.pattern.len());
            for range in self.pattern.iter() {
                write!(rope, "{}", range)?;
            }

            match self.miss.is_none() {
                false => {
                    let mut list = f.debug_list();

                    list.entry(&Arm(rope, self.then));
                    list.entry(&Arm('_', self.miss));

                    list.finish()
                }
                true => Arm(rope, self.then).fmt(f),
            }
        }
    }

    impl PartialEq for Fork {
        fn eq(&self, other: &Self) -> bool {
            self.miss == other.miss && self.branches().eq(other.branches())
        }
    }

    impl<T: Debug> Debug for Node<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Node::Fork(fork) => fork.fmt(f),
                Node::Rope(rope) => rope.fmt(f),
                Node::Leaf(leaf) => leaf.fmt(f),
            }
        }
    }

    use std::ops::RangeInclusive;

    impl From<RangeInclusive<u8>> for Range {
        fn from(range: RangeInclusive<u8>) -> Range {
            Range {
                start: *range.start(),
                end: *range.end(),
            }
        }
    }

    impl From<RangeInclusive<char>> for Range {
        fn from(range: RangeInclusive<char>) -> Range {
            Range {
                start: *range.start() as u8,
                end: *range.end() as u8,
            }
        }
    }

    impl<T> PartialEq<Rope> for Node<T> {
        fn eq(&self, other: &Rope) -> bool {
            match self {
                Node::Rope(rope) => rope == other,
                _ => false,
            }
        }
    }

    impl<T> PartialEq<Fork> for Node<T> {
        fn eq(&self, other: &Fork) -> bool {
            match self {
                Node::Fork(fork) => fork == other,
                _ => false,
            }
        }
    }
}
