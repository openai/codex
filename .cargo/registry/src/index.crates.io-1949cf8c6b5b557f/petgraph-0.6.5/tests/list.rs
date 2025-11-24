extern crate itertools;
extern crate petgraph;
#[macro_use]
extern crate defmac;

use petgraph::adj::DefaultIx;
use petgraph::adj::IndexType;
use petgraph::adj::{List, UnweightedList};
use petgraph::algo::tarjan_scc;
use petgraph::data::{DataMap, DataMapMut};
use petgraph::dot::Dot;
use petgraph::prelude::*;
use petgraph::visit::{
    IntoEdgeReferences, IntoEdges, IntoNeighbors, IntoNodeReferences, NodeCount, NodeIndexable,
};

use itertools::assert_equal;

fn n(x: u32) -> DefaultIx {
    DefaultIx::new(x as _)
}

#[test]
fn node_indices() {
    let mut g = List::<()>::new();
    let a = g.add_node();
    let b = g.add_node();
    let c = g.add_node();
    let mut iter = g.node_indices();
    assert_eq!(iter.next(), Some(a));
    assert_eq!(iter.next(), Some(b));
    assert_eq!(iter.next(), Some(c));
    assert_eq!(iter.next(), None);
}

fn test_node_count<E>(g: &List<E>, n: usize) {
    assert_eq!(n, g.node_count());
    assert_eq!(g.node_bound(), n);
    assert_eq!(g.node_indices().count(), n);
    assert_eq!(g.node_indices().len(), n);
    assert_eq!(g.node_references().count(), n);
    assert_eq!(g.node_references().len(), n);
}

#[test]
fn node_bound() {
    let mut g = List::<()>::new();
    test_node_count(&g, 0);
    for i in 0..10 {
        g.add_node();
        test_node_count(&g, i + 1);
    }
    g.clear();
    test_node_count(&g, 0);
}

fn assert_sccs_eq<Ix: IndexType>(mut res: Vec<Vec<Ix>>, normalized: Vec<Vec<Ix>>) {
    // normalize the result and compare with the answer.
    for scc in &mut res {
        scc.sort();
    }
    // sort by minimum element
    res.sort_by(|v, w| v[0].cmp(&w[0]));
    assert_eq!(res, normalized);
}

fn scc_graph() -> UnweightedList<DefaultIx> {
    let mut gr = List::new();
    for _ in 0..9 {
        gr.add_node();
    }
    for (a, b) in &[
        (6, 0),
        (0, 3),
        (3, 6),
        (8, 6),
        (8, 2),
        (2, 5),
        (5, 8),
        (7, 5),
        (1, 7),
    ] {
        gr.add_edge(n(*a), n(*b), ());
    }
    // make an identical replacement of n(4) and leave a hole
    let x = gr.add_node();
    gr.add_edge(n(7), x, ());
    gr.add_edge(x, n(1), ());
    gr
}

#[test]
fn test_tarjan_scc() {
    let gr = scc_graph();

    let x = n(gr.node_bound() as u32 - 1);
    assert_sccs_eq(
        tarjan_scc(&gr),
        vec![
            vec![n(0), n(3), n(6)],
            vec![n(1), n(7), x],
            vec![n(2), n(5), n(8)],
            vec![n(4)],
        ],
    );
}

fn make_graph() -> List<i32> {
    let mut gr = List::new();
    let mut c = 0..;
    let mut e = || -> i32 { c.next().unwrap() };
    for _ in 0..=9 {
        gr.add_node();
    }
    for &(from, to) in &[
        (6, 0),
        (0, 3),
        (3, 6),
        (8, 6),
        (8, 2),
        (2, 5),
        (5, 8),
        (7, 5),
        (1, 7),
        (7, 9),
        (8, 6), // parallel edge
        (9, 1),
        (9, 9),
        (9, 9),
    ] {
        gr.add_edge(n(from), n(to), e());
    }
    gr
}

defmac!(edges ref gr, x => gr.edges(x).map(|r| (r.target(), *r.weight())));

#[test]
fn test_edges_directed() {
    let gr = make_graph();
    dbg!(&gr);
    let x = n(9);
    assert_equal(edges!(&gr, x), vec![(1, 11), (x, 12), (x, 13)]);
    assert_equal(edges!(&gr, n(0)), vec![(n(3), 1)]);
    //assert_equal(edges!(&gr, n(4)), vec![]);
}

#[test]
fn test_edge_references() {
    let mut gr = make_graph();
    assert_eq!(gr.edge_count(), gr.edge_references().count());
    for i in gr.edge_references() {
        assert_eq!(gr.edge_endpoints(i.id()), Some((i.source(), i.target())));
        assert_eq!(gr.edge_weight(i.id()), Some(i.weight()));
    }
    for n in gr.node_indices() {
        for e in gr.edge_indices_from(n) {
            match gr.edge_weight_mut(e) {
                None => {}
                Some(r) => {
                    *r = 1;
                }
            }
        }
    }
    for i in gr.edge_references() {
        assert_eq!(*i.weight(), 1);
    }
}

#[test]
fn test_edge_iterators() {
    let gr = make_graph();
    for i in gr.node_indices() {
        itertools::assert_equal(
            gr.neighbors(n(i)),
            gr.edges(n(i)).map(|r| {
                assert_eq!(r.source(), n(i));
                r.target()
            }),
        );
    }
}

#[test]
#[should_panic(expected = "is not a valid node")]
fn add_edge_vacant() {
    let mut g = List::new();
    let a: DefaultIx = g.add_node();
    let b = g.add_node();
    let _ = g.add_node();
    g.clear();
    g.add_edge(a, b, 1);
}

#[test]
#[should_panic(expected = "is not a valid node")]
fn add_edge_oob() {
    let mut g = List::new();
    let a = g.add_node();
    let _ = g.add_node();
    let _ = g.add_node();
    g.add_edge(a, n(4), 1);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn add_edge_oob_2() {
    let mut g = List::new();
    let a = g.add_node();
    let _ = g.add_node();
    let _ = g.add_node();
    g.add_edge(n(4), a, 1);
}

#[test]
fn test_node_references() {
    let gr = scc_graph();

    itertools::assert_equal(gr.node_references(), gr.node_indices());
}

#[test]
fn iterators_undir() {
    let mut g = List::with_capacity(2);
    let a = g.add_node();
    let b = g.add_node();
    let c = g.add_node();
    let d = g.add_node();
    for &(from, to, w) in &[(a, b, 1), (a, c, 2), (b, c, 3), (c, c, 4), (a, d, 5)] {
        g.add_edge(n(from), n(to), w);
    }

    itertools::assert_equal(g.neighbors(a), vec![b, c, d]);
    itertools::assert_equal(g.neighbors(c), vec![c]);
    itertools::assert_equal(g.neighbors(d), vec![]);

    itertools::assert_equal(g.neighbors(b), vec![c]);
}

#[test]
fn dot() {
    let mut gr = List::new();
    let a: DefaultIx = gr.add_node();
    let b = gr.add_node();
    gr.add_edge(a, a, 10u8);
    gr.add_edge(a, b, 20);
    let dot_output = format!("{:?}", Dot::new(&gr));
    assert_eq!(
        dot_output,
        r#"digraph {
    0 [ label = "()" ]
    1 [ label = "()" ]
    0 -> 0 [ label = "10" ]
    0 -> 1 [ label = "20" ]
}
"#
    );
}
