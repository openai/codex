use std::collections::HashSet;
use std::hash::Hash;

use petgraph::algo::{greedy_matching, maximum_matching};
use petgraph::prelude::*;

macro_rules! assert_one_of {
    ($actual:expr, [$($expected:expr),+]) => {
        let expected = &[$($expected),+];
        if !expected.iter().any(|expected| expected == &$actual) {
            let expected = expected.iter().map(|e| format!("\n{:?}", e)).collect::<Vec<_>>();
            let comma_separated = expected.join(", ");
            panic!("assertion failed: `actual does not equal to any of expected`\nactual:\n{:?}\nexpected:{}", $actual, comma_separated);
        }
    };
}

macro_rules! set {
    () => {
        HashSet::new()
    };
    ($(($source:expr, $target:expr)),+) => {
        {
            let mut set = HashSet::new();
            $(
                set.insert(($source.into(), $target.into()));
            )*
            set
        }
    };
    ($($elem:expr),+) => {
        {
            let mut set = HashSet::new();
            $(
                set.insert($elem.into());
            )*
            set
        }
    };
}

// So we don't have to type `.collect::<HashSet<_>>`.
fn collect<'a, T: Copy + Eq + Hash + 'a>(iter: impl Iterator<Item = T>) -> HashSet<T> {
    iter.collect()
}

#[test]
fn greedy_empty() {
    let g: UnGraph<(), ()> = UnGraph::default();
    let m = greedy_matching(&g);
    assert_eq!(collect(m.edges()), set![]);
    assert_eq!(collect(m.nodes()), set![]);
}

#[test]
fn greedy_disjoint() {
    let g: UnGraph<(), ()> = UnGraph::from_edges(&[(0, 1), (2, 3)]);
    let m = greedy_matching(&g);
    assert_eq!(collect(m.edges()), set![(0, 1), (2, 3)]);
    assert_eq!(collect(m.nodes()), set![0, 1, 2, 3]);
}

#[test]
fn greedy_odd_path() {
    let g: UnGraph<(), ()> = UnGraph::from_edges(&[(0, 1), (1, 2), (2, 3)]);
    let m = greedy_matching(&g);
    assert_one_of!(collect(m.edges()), [set![(0, 1), (2, 3)], set![(1, 2)]]);
    assert_one_of!(collect(m.nodes()), [set![0, 1, 2, 3], set![1, 2]]);
}

#[test]
fn greedy_star() {
    let g: UnGraph<(), ()> = UnGraph::from_edges(&[(0, 1), (0, 2), (0, 3)]);
    let m = greedy_matching(&g);
    assert_one_of!(
        collect(m.edges()),
        [set![(0, 1)], set![(0, 2)], set![(0, 3)]]
    );
    assert_one_of!(collect(m.nodes()), [set![0, 1], set![0, 2], set![0, 3]]);
}

#[test]
fn maximum_empty() {
    let g: UnGraph<(), ()> = UnGraph::default();
    let m = maximum_matching(&g);
    assert_eq!(collect(m.edges()), set![]);
    assert_eq!(collect(m.nodes()), set![]);
}

#[test]
fn maximum_disjoint() {
    let g: UnGraph<(), ()> = UnGraph::from_edges(&[(0, 1), (2, 3)]);
    let m = maximum_matching(&g);
    assert_eq!(collect(m.edges()), set![(0, 1), (2, 3)]);
    assert_eq!(collect(m.nodes()), set![0, 1, 2, 3]);
}

#[test]
fn maximum_odd_path() {
    let g: UnGraph<(), ()> = UnGraph::from_edges(&[(0, 1), (1, 2), (2, 3)]);
    let m = maximum_matching(&g);
    assert_eq!(collect(m.edges()), set![(0, 1), (2, 3)]);
    assert_eq!(collect(m.nodes()), set![0, 1, 2, 3]);
}

#[cfg(feature = "stable_graph")]
#[test]
fn maximum_in_stable_graph() {
    let mut g: StableUnGraph<(), ()> =
        StableUnGraph::from_edges(&[(0, 1), (0, 2), (1, 2), (1, 3), (2, 4), (3, 4), (3, 5)]);

    // Create a hole by removing node that would otherwise belong to the maximum
    // matching.
    g.remove_node(NodeIndex::new(4));

    let m = maximum_matching(&g);
    assert_one_of!(
        collect(m.edges()),
        [
            set![(0, 1), (3, 5)],
            set![(0, 2), (1, 3)],
            set![(0, 2), (3, 5)]
        ]
    );
    assert_one_of!(
        collect(m.nodes()),
        [set![0, 1, 3, 5], set![0, 2, 1, 3], set![0, 2, 3, 5]]
    );
}

#[cfg(feature = "stable_graph")]
#[test]
fn is_perfect_in_stable_graph() {
    let mut g: StableUnGraph<(), ()> = StableUnGraph::from_edges(&[(0, 1), (1, 2), (2, 3)]);
    g.remove_node(NodeIndex::new(0));
    g.remove_node(NodeIndex::new(1));

    let m = maximum_matching(&g);
    assert_eq!(m.len(), 1);
    assert!(m.is_perfect());
}
