#![cfg(feature = "stable_graph")]

extern crate itertools;
extern crate petgraph;
#[macro_use]
extern crate defmac;

use std::collections::HashSet;

use itertools::assert_equal;
use petgraph::algo::{kosaraju_scc, min_spanning_tree, tarjan_scc};
use petgraph::dot::Dot;
use petgraph::prelude::*;
use petgraph::stable_graph::edge_index as e;
use petgraph::stable_graph::node_index as n;
use petgraph::visit::{EdgeIndexable, IntoEdgeReferences, IntoNodeReferences, NodeIndexable};
use petgraph::EdgeType;

#[test]
fn node_indices() {
    let mut g = StableGraph::<_, ()>::new();
    let a = g.add_node(0);
    let b = g.add_node(1);
    let c = g.add_node(2);
    g.remove_node(b);
    let mut iter = g.node_indices();
    assert_eq!(iter.next(), Some(a));
    assert_eq!(iter.next(), Some(c));
    assert_eq!(iter.next(), None);
}

#[test]
fn node_bound() {
    let mut g = StableGraph::<_, ()>::new();
    assert_eq!(g.node_bound(), g.node_count());
    for i in 0..10 {
        g.add_node(i);
        assert_eq!(g.node_bound(), g.node_count());
    }
    let full_count = g.node_count();
    g.remove_node(n(0));
    g.remove_node(n(2));
    assert_eq!(g.node_bound(), full_count);
    g.clear();
    assert_eq!(g.node_bound(), 0);
}

#[test]
fn edge_bound() {
    let mut g = StableGraph::<_, _>::new();
    assert_eq!(g.edge_bound(), g.edge_count());
    for i in 0..10 {
        g.add_node(i);
    }
    for i in 0..9 {
        g.add_edge(n(i), n(i + 1), i);
        assert_eq!(g.edge_bound(), g.edge_count());
    }
    let full_count = g.edge_count();
    g.remove_edge(e(0));
    g.remove_edge(e(2));
    assert_eq!(g.edge_bound(), full_count);
    g.clear();
    assert_eq!(g.edge_bound(), 0);
}

#[test]
fn clear_edges() {
    let mut gr = scc_graph();
    gr.remove_node(n(1));
    gr.clear_edges();
    // check that we use the free list for the vacancies
    assert_eq!(gr.add_node(()), n(1));
    assert_eq!(gr.add_node(()), n(4));
    assert!(gr.edge_references().next().is_none());
    assert!(gr.node_indices().all(|i| gr.neighbors(i).next().is_none()));
}

fn assert_sccs_eq(mut res: Vec<Vec<NodeIndex>>, normalized: Vec<Vec<NodeIndex>>) {
    // normalize the result and compare with the answer.
    for scc in &mut res {
        scc.sort();
    }
    // sort by minimum element
    res.sort_by(|v, w| v[0].cmp(&w[0]));
    assert_eq!(res, normalized);
}

fn scc_graph() -> StableGraph<(), ()> {
    let mut gr: StableGraph<(), ()> = StableGraph::from_edges(&[
        (6, 0),
        (0, 3),
        (3, 6),
        (8, 6),
        (8, 2),
        (2, 5),
        (5, 8),
        (7, 5),
        (1, 7),
        (7, 4),
        (4, 1),
    ]);
    // make an identical replacement of n(4) and leave a hole
    let x = gr.add_node(());
    gr.add_edge(n(7), x, ());
    gr.add_edge(x, n(1), ());
    gr.remove_node(n(4));
    gr
}

#[test]
fn test_scc() {
    let gr = scc_graph();
    println!("{:?}", gr);

    let x = n(gr.node_bound() - 1);
    assert_sccs_eq(
        kosaraju_scc(&gr),
        vec![
            vec![n(0), n(3), n(6)],
            vec![n(1), n(7), x],
            vec![n(2), n(5), n(8)],
        ],
    );
}

#[test]
fn test_tarjan_scc() {
    let gr = scc_graph();

    let x = n(gr.node_bound() - 1);
    assert_sccs_eq(
        tarjan_scc(&gr),
        vec![
            vec![n(0), n(3), n(6)],
            vec![n(1), n(7), x],
            vec![n(2), n(5), n(8)],
        ],
    );
}

fn make_graph<Ty>() -> StableGraph<(), i32, Ty>
where
    Ty: EdgeType,
{
    let mut gr = StableGraph::default();
    let mut c = 0..;
    let mut e = || -> i32 { c.next().unwrap() };
    gr.extend_with_edges(&[
        (6, 0, e()),
        (0, 3, e()),
        (3, 6, e()),
        (8, 6, e()),
        (8, 2, e()),
        (2, 5, e()),
        (5, 8, e()),
        (7, 5, e()),
        (1, 7, e()),
        (7, 4, e()),
        (8, 6, e()), // parallel edge
        (4, 1, e()),
    ]);
    // make an identical replacement of n(4) and leave a hole
    let x = gr.add_node(());
    gr.add_edge(n(7), x, e());
    gr.add_edge(x, n(1), e());
    gr.add_edge(x, x, e()); // make two self loops
    let rm_self_loop = gr.add_edge(x, x, e());
    gr.add_edge(x, x, e());
    gr.remove_node(n(4));
    gr.remove_node(n(6));
    gr.remove_edge(rm_self_loop);
    gr
}

defmac!(edges ref gr, x => gr.edges(x).map(|r| (r.target(), *r.weight())));

#[test]
fn test_edges_directed() {
    let gr = make_graph::<Directed>();
    let x = n(9);
    assert_equal(edges!(&gr, x), vec![(x, 16), (x, 14), (n(1), 13)]);
    assert_equal(edges!(&gr, n(0)), vec![(n(3), 1)]);
    assert_equal(edges!(&gr, n(4)), vec![]);
}

#[test]
fn test_edge_references() {
    let gr = make_graph::<Directed>();
    assert_eq!(gr.edge_count(), gr.edge_references().count());
}

#[test]
fn test_edges_undirected() {
    let gr = make_graph::<Undirected>();
    let x = n(9);
    assert_equal(
        edges!(&gr, x),
        vec![(x, 16), (x, 14), (n(1), 13), (n(7), 12)],
    );
    assert_equal(edges!(&gr, n(0)), vec![(n(3), 1)]);
    assert_equal(edges!(&gr, n(4)), vec![]);
}

#[test]
fn test_edge_iterators_directed() {
    let gr = make_graph::<Directed>();
    for i in gr.node_indices() {
        itertools::assert_equal(gr.edges_directed(i, Outgoing), gr.edges(i));
        for edge in gr.edges_directed(i, Outgoing) {
            assert_eq!(
                edge.source(),
                i,
                "outgoing edges should have a fixed source"
            );
        }
    }
    let mut incoming = vec![Vec::new(); gr.node_bound()];

    for i in gr.node_indices() {
        for j in gr.neighbors(i) {
            incoming[j.index()].push(i);
        }
    }

    println!("{:#?}", gr);
    for i in gr.node_indices() {
        itertools::assert_equal(
            gr.edges_directed(i, Incoming).map(|e| e.source()),
            incoming[i.index()].iter().rev().cloned(),
        );
        for edge in gr.edges_directed(i, Incoming) {
            assert_eq!(
                edge.target(),
                i,
                "incoming edges should have a fixed target"
            );
        }
    }
}

#[test]
fn test_edge_iterators_undir() {
    let gr = make_graph::<Undirected>();
    for i in gr.node_indices() {
        itertools::assert_equal(gr.edges_directed(i, Outgoing), gr.edges(i));
        for edge in gr.edges_directed(i, Outgoing) {
            assert_eq!(
                edge.source(),
                i,
                "outgoing edges should have a fixed source"
            );
        }
    }
    for i in gr.node_indices() {
        itertools::assert_equal(gr.edges_directed(i, Incoming), gr.edges(i));
        for edge in gr.edges_directed(i, Incoming) {
            assert_eq!(
                edge.target(),
                i,
                "incoming edges should have a fixed target"
            );
        }
    }
}

#[test]
#[should_panic(expected = "is not a node")]
fn add_edge_vacant() {
    let mut g = StableGraph::<_, _>::new();
    let a = g.add_node(0);
    let b = g.add_node(1);
    let _ = g.add_node(2);
    let _ = g.remove_node(b);
    g.add_edge(a, b, 1);
}

#[test]
#[should_panic(expected = "is not a node")]
fn add_edge_oob() {
    let mut g = StableGraph::<_, _>::new();
    let a = g.add_node(0);
    let _ = g.add_node(1);
    let _ = g.add_node(2);
    g.add_edge(a, n(4), 1);
}

#[test]
fn test_node_references() {
    let gr = scc_graph();

    itertools::assert_equal(gr.node_references().map(|(i, _)| i), gr.node_indices());
}

#[test]
fn iterators_undir() {
    let mut g = StableUnGraph::<_, _>::default();
    let a = g.add_node(0);
    let b = g.add_node(1);
    let c = g.add_node(2);
    let d = g.add_node(3);
    g.extend_with_edges(&[(a, b, 1), (a, c, 2), (b, c, 3), (c, c, 4), (a, d, 5)]);
    g.remove_node(b);

    itertools::assert_equal(g.neighbors(a), vec![d, c]);
    itertools::assert_equal(g.neighbors(c), vec![c, a]);
    itertools::assert_equal(g.neighbors(d), vec![a]);

    // the node that was removed
    itertools::assert_equal(g.neighbors(b), vec![]);

    // remove one more
    g.remove_node(c);
    itertools::assert_equal(g.neighbors(c), vec![]);
}

#[test]
fn iter_multi_edges() {
    let mut gr = StableGraph::new();
    let a = gr.add_node("a");
    let b = gr.add_node("b");
    let c = gr.add_node("c");

    let mut connecting_edges = HashSet::new();

    gr.add_edge(a, a, ());
    connecting_edges.insert(gr.add_edge(a, b, ()));
    gr.add_edge(a, c, ());
    gr.add_edge(c, b, ());
    connecting_edges.insert(gr.add_edge(a, b, ()));
    gr.add_edge(b, a, ());

    let mut iter = gr.edges_connecting(a, b);

    let edge_id = iter.next().unwrap().id();
    assert!(connecting_edges.contains(&edge_id));
    connecting_edges.remove(&edge_id);

    let edge_id = iter.next().unwrap().id();
    assert!(connecting_edges.contains(&edge_id));
    connecting_edges.remove(&edge_id);

    assert_eq!(None, iter.next());
    assert!(connecting_edges.is_empty());
}

#[test]
fn iter_multi_undirected_edges() {
    let mut gr: StableUnGraph<_, _> = Default::default();
    let a = gr.add_node("a");
    let b = gr.add_node("b");
    let c = gr.add_node("c");

    let mut connecting_edges = HashSet::new();

    gr.add_edge(a, a, ());
    connecting_edges.insert(gr.add_edge(a, b, ()));
    gr.add_edge(a, c, ());
    gr.add_edge(c, b, ());
    connecting_edges.insert(gr.add_edge(a, b, ()));
    connecting_edges.insert(gr.add_edge(b, a, ()));

    let mut iter = gr.edges_connecting(a, b);

    let edge_id = iter.next().unwrap().id();
    assert!(connecting_edges.contains(&edge_id));
    connecting_edges.remove(&edge_id);

    let edge_id = iter.next().unwrap().id();
    assert!(connecting_edges.contains(&edge_id));
    connecting_edges.remove(&edge_id);

    let edge_id = iter.next().unwrap().id();
    assert!(connecting_edges.contains(&edge_id));
    connecting_edges.remove(&edge_id);

    assert_eq!(None, iter.next());
    assert!(connecting_edges.is_empty());
}

#[test]
fn dot() {
    let mut gr = StableGraph::new();
    let a = gr.add_node("x");
    let b = gr.add_node("y");
    gr.add_edge(a, a, "10");
    gr.add_edge(a, b, "20");
    let dot_output = format!("{}", Dot::new(&gr));
    assert_eq!(
        dot_output,
        r#"digraph {
    0 [ label = "x" ]
    1 [ label = "y" ]
    0 -> 0 [ label = "10" ]
    0 -> 1 [ label = "20" ]
}
"#
    );
}

defmac!(iter_eq a, b => a.eq(b));
defmac!(nodes_eq ref a, ref b => a.node_references().eq(b.node_references()));
defmac!(edgew_eq ref a, ref b => a.edge_references().eq(b.edge_references()));
defmac!(edges_eq ref a, ref b =>
        iter_eq!(
            a.edge_references().map(|e| (e.source(), e.target())),
            b.edge_references().map(|e| (e.source(), e.target()))));

#[test]
fn from() {
    let mut gr1 = StableGraph::new();
    let a = gr1.add_node(1);
    let b = gr1.add_node(2);
    let c = gr1.add_node(3);
    gr1.add_edge(a, a, 10);
    gr1.add_edge(a, b, 20);
    gr1.add_edge(b, c, 30);
    gr1.add_edge(a, c, 40);

    let gr2 = Graph::from(gr1.clone());
    let gr3 = StableGraph::from(gr2);
    assert!(nodes_eq!(&gr1, &gr3));
    assert!(edgew_eq!(&gr1, &gr3));
    assert!(edges_eq!(&gr1, &gr3));

    gr1.remove_node(b);

    let gr4 = Graph::from(gr1);
    let gr5 = StableGraph::from(gr4.clone());

    let mut ans = StableGraph::new();
    let a = ans.add_node(1);
    let c = ans.add_node(3);
    ans.add_edge(a, a, 10);
    ans.add_edge(a, c, 40);

    assert!(nodes_eq!(&gr4, &ans));
    assert!(edges_eq!(&gr4, &ans));

    assert!(nodes_eq!(&gr5, &ans));
    assert!(edgew_eq!(&gr5, &ans));
    assert!(edges_eq!(&gr5, &ans));
}

use petgraph::data::FromElements;
use petgraph::stable_graph::StableGraph;

#[test]
fn from_min_spanning_tree() {
    let mut g = StableGraph::new();
    let mut nodes = Vec::new();
    for _ in 0..6 {
        nodes.push(g.add_node(()));
    }
    let es = [(4, 5), (3, 4), (3, 5)];
    for &(a, b) in es.iter() {
        g.add_edge(NodeIndex::new(a), NodeIndex::new(b), ());
    }
    for i in 0..3 {
        let _ = g.remove_node(nodes[i]);
    }
    let _ = StableGraph::<(), (), Undirected, usize>::from_elements(min_spanning_tree(&g));
}

#[test]
fn weights_mut_iterator() {
    let mut gr = StableGraph::new();
    let a = gr.add_node(1);
    let b = gr.add_node(2);
    let c = gr.add_node(3);
    let e1 = gr.add_edge(a, a, 10);
    let e2 = gr.add_edge(a, b, 20);
    let e3 = gr.add_edge(b, c, 30);
    let e4 = gr.add_edge(a, c, 40);

    for n in gr.node_weights_mut() {
        *n += 1;
    }
    assert_eq!(gr[a], 2);
    assert_eq!(gr[b], 3);
    assert_eq!(gr[c], 4);

    for e in gr.edge_weights_mut() {
        *e -= 1;
    }
    assert_eq!(gr[e1], 9);
    assert_eq!(gr[e2], 19);
    assert_eq!(gr[e3], 29);
    assert_eq!(gr[e4], 39);

    // test on deletion
    gr.remove_node(b);
    assert_eq!(gr.node_weights_mut().count(), gr.node_count());
    assert_eq!(gr.edge_weights_mut().count(), gr.edge_count());
}
