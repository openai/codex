extern crate petgraph;

use std::collections::HashSet;
use std::hash::Hash;

use petgraph::prelude::*;
use petgraph::EdgeType;

use petgraph as pg;

use petgraph::algo::{
    dominators, has_path_connecting, is_bipartite_undirected, is_cyclic_undirected,
    is_isomorphic_matching,
};

use petgraph::graph::node_index as n;
use petgraph::graph::IndexType;

use petgraph::algo::{astar, dijkstra, DfsSpace};
use petgraph::visit::{
    IntoEdges, IntoEdgesDirected, IntoNeighbors, IntoNodeIdentifiers, NodeFiltered, Reversed, Topo,
    VisitMap, Walker,
};

use petgraph::dot::Dot;

fn set<I>(iter: I) -> HashSet<I::Item>
where
    I: IntoIterator,
    I::Item: Hash + Eq,
{
    iter.into_iter().collect()
}

#[test]
fn undirected() {
    let mut og = Graph::new_undirected();
    let a = og.add_node(0);
    let b = og.add_node(1);
    let c = og.add_node(2);
    let d = og.add_node(3);
    let _ = og.add_edge(a, b, 0);
    let _ = og.add_edge(a, c, 1);
    og.add_edge(c, a, 2);
    og.add_edge(a, a, 3);
    og.add_edge(b, c, 4);
    og.add_edge(b, a, 5);
    og.add_edge(a, d, 6);
    assert_eq!(og.node_count(), 4);
    assert_eq!(og.edge_count(), 7);

    assert!(og.find_edge(a, b).is_some());
    assert!(og.find_edge(d, a).is_some());
    assert!(og.find_edge(a, a).is_some());

    for edge in og.raw_edges() {
        assert!(og.find_edge(edge.source(), edge.target()).is_some());
        assert!(og.find_edge(edge.target(), edge.source()).is_some());
    }

    assert_eq!(og.neighbors(b).collect::<Vec<_>>(), vec![a, c, a]);

    og.remove_node(a);
    assert_eq!(og.neighbors(b).collect::<Vec<_>>(), vec![c]);
    assert_eq!(og.node_count(), 3);
    assert_eq!(og.edge_count(), 1);
    assert!(og.find_edge(a, b).is_none());
    assert!(og.find_edge(d, a).is_none());
    assert!(og.find_edge(a, a).is_none());
    assert!(og.find_edge(b, c).is_some());
    assert_graph_consistent(&og);
}

#[test]
fn dfs() {
    let mut gr = Graph::new();
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    let k = gr.add_node("K");
    // Z is disconnected.
    let _ = gr.add_node("Z");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);
    gr.add_edge(i, k, 2.);

    println!("{}", Dot::new(&gr));

    assert_eq!(Dfs::new(&gr, h).iter(&gr).count(), 4);
    assert_eq!(Dfs::new(&gr, h).iter(&gr).clone().count(), 4);

    assert_eq!(Dfs::new(&gr, h).iter(Reversed(&gr)).count(), 1);

    assert_eq!(Dfs::new(&gr, k).iter(Reversed(&gr)).count(), 3);

    assert_eq!(Dfs::new(&gr, i).iter(&gr).count(), 3);
}

#[test]
fn dfs_order() {
    let mut gr = Graph::new();
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    let k = gr.add_node("K");
    gr.add_edge(h, i, ());
    gr.add_edge(h, j, ());
    gr.add_edge(h, k, ());
    gr.add_edge(i, k, ());
    gr.add_edge(k, i, ());

    //      H
    //    / | \
    //   I  J  K
    //    \___/
    //
    // J cannot be visited between I and K in a depth-first search from H

    let mut seen_i = false;
    let mut seen_k = false;
    for node in Dfs::new(&gr, h).iter(&gr) {
        seen_i |= i == node;
        seen_k |= k == node;
        assert!(!(j == node && (seen_i ^ seen_k)), "Invalid DFS order");
    }
}

#[test]
fn bfs() {
    let mut gr = Graph::new();
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    let k = gr.add_node("K");
    // Z is disconnected.
    let _ = gr.add_node("Z");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);
    gr.add_edge(i, k, 2.);

    assert_eq!(Bfs::new(&gr, h).iter(&gr).count(), 4);
    assert_eq!(Bfs::new(&gr, h).iter(&gr).clone().count(), 4);

    assert_eq!(Bfs::new(&gr, h).iter(Reversed(&gr)).count(), 1);

    assert_eq!(Bfs::new(&gr, k).iter(Reversed(&gr)).count(), 3);

    assert_eq!(Bfs::new(&gr, i).iter(&gr).count(), 3);

    let mut bfs = Bfs::new(&gr, h);
    let nx = bfs.next(&gr);
    assert_eq!(nx, Some(h));

    let nx1 = bfs.next(&gr);
    assert!(nx1 == Some(i) || nx1 == Some(j));

    let nx2 = bfs.next(&gr);
    assert!(nx2 == Some(i) || nx2 == Some(j));
    assert!(nx1 != nx2);

    let nx = bfs.next(&gr);
    assert_eq!(nx, Some(k));
    assert_eq!(bfs.next(&gr), None);
}

#[test]
fn selfloop() {
    let mut gr = Graph::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    gr.add_edge(a, b, 7.);
    gr.add_edge(c, a, 6.);
    let sed = gr.add_edge(a, a, 2.);

    assert!(gr.find_edge(a, b).is_some());
    assert!(gr.find_edge(b, a).is_none());
    assert!(gr.find_edge_undirected(b, a).is_some());
    assert!(gr.find_edge(a, a).is_some());
    println!("{:?}", gr);

    gr.remove_edge(sed);
    assert!(gr.find_edge(a, a).is_none());
    println!("{:?}", gr);
}

#[test]
fn cyclic() {
    let mut gr = Graph::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");

    assert!(!is_cyclic_undirected(&gr));
    gr.add_edge(a, b, 7.);
    gr.add_edge(c, a, 6.);
    assert!(!is_cyclic_undirected(&gr));
    {
        let e = gr.add_edge(a, a, 0.);
        assert!(is_cyclic_undirected(&gr));
        gr.remove_edge(e);
        assert!(!is_cyclic_undirected(&gr));
    }

    {
        let e = gr.add_edge(b, c, 0.);
        assert!(is_cyclic_undirected(&gr));
        gr.remove_edge(e);
        assert!(!is_cyclic_undirected(&gr));
    }

    let d = gr.add_node("D");
    let e = gr.add_node("E");
    gr.add_edge(b, d, 0.);
    gr.add_edge(d, e, 0.);
    assert!(!is_cyclic_undirected(&gr));
    gr.add_edge(c, e, 0.);
    assert!(is_cyclic_undirected(&gr));
    assert_graph_consistent(&gr);
}

#[test]
fn bipartite() {
    {
        let mut gr = Graph::new_undirected();
        let a = gr.add_node("A");
        let b = gr.add_node("B");
        let c = gr.add_node("C");

        let d = gr.add_node("D");
        let e = gr.add_node("E");
        let f = gr.add_node("F");

        gr.add_edge(a, d, 7.);
        gr.add_edge(b, d, 6.);

        assert!(is_bipartite_undirected(&gr, a));

        let e_index = gr.add_edge(a, b, 6.);

        assert!(!is_bipartite_undirected(&gr, a));

        gr.remove_edge(e_index);

        assert!(is_bipartite_undirected(&gr, a));

        gr.add_edge(b, e, 7.);
        gr.add_edge(b, f, 6.);
        gr.add_edge(c, e, 6.);

        assert!(is_bipartite_undirected(&gr, a));
    }
    {
        let mut gr = Graph::new_undirected();
        let a = gr.add_node("A");
        let b = gr.add_node("B");
        let c = gr.add_node("C");
        let d = gr.add_node("D");
        let e = gr.add_node("E");

        let f = gr.add_node("F");
        let g = gr.add_node("G");
        let h = gr.add_node("H");

        gr.add_edge(a, f, 7.);
        gr.add_edge(a, g, 7.);
        gr.add_edge(a, h, 7.);

        gr.add_edge(b, f, 6.);
        gr.add_edge(b, g, 6.);
        gr.add_edge(b, h, 6.);

        gr.add_edge(c, f, 6.);
        gr.add_edge(c, g, 6.);
        gr.add_edge(c, h, 6.);

        gr.add_edge(d, f, 6.);
        gr.add_edge(d, g, 6.);
        gr.add_edge(d, h, 6.);

        gr.add_edge(e, f, 6.);
        gr.add_edge(e, g, 6.);
        gr.add_edge(e, h, 6.);

        assert!(is_bipartite_undirected(&gr, a));

        gr.add_edge(a, b, 6.);

        assert!(!is_bipartite_undirected(&gr, a));
    }
}

#[test]
fn multi() {
    let mut gr = Graph::new();
    let a = gr.add_node("a");
    let b = gr.add_node("b");
    gr.add_edge(a, b, ());
    gr.add_edge(a, b, ());
    assert_eq!(gr.edge_count(), 2);
}

#[test]
fn iter_multi_edges() {
    let mut gr = Graph::new();
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
    let mut gr = Graph::new_undirected();
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
fn update_edge() {
    {
        let mut gr = Graph::new();
        let a = gr.add_node("a");
        let b = gr.add_node("b");
        let e = gr.update_edge(a, b, 1);
        let f = gr.update_edge(a, b, 2);
        let _ = gr.update_edge(b, a, 3);
        assert_eq!(gr.edge_count(), 2);
        assert_eq!(e, f);
        assert_eq!(*gr.edge_weight(f).unwrap(), 2);
    }

    {
        let mut gr = Graph::new_undirected();
        let a = gr.add_node("a");
        let b = gr.add_node("b");
        let e = gr.update_edge(a, b, 1);
        let f = gr.update_edge(b, a, 2);
        assert_eq!(gr.edge_count(), 1);
        assert_eq!(e, f);
        assert_eq!(*gr.edge_weight(f).unwrap(), 2);
    }
}

#[test]
fn dijk() {
    let mut g = Graph::new_undirected();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    let f = g.add_node("F");
    g.add_edge(a, b, 7);
    g.add_edge(c, a, 9);
    g.add_edge(a, d, 14);
    g.add_edge(b, c, 10);
    g.add_edge(d, c, 2);
    g.add_edge(d, e, 9);
    g.add_edge(b, f, 15);
    g.add_edge(c, f, 11);
    g.add_edge(e, f, 6);
    println!("{:?}", g);
    for no in Bfs::new(&g, a).iter(&g) {
        println!("Visit {:?} = {:?}", no, g.node_weight(no));
    }

    let scores = dijkstra(&g, a, None, |e| *e.weight());
    let mut scores: Vec<_> = scores.into_iter().map(|(n, s)| (g[n], s)).collect();
    scores.sort();
    assert_eq!(
        scores,
        vec![
            ("A", 0),
            ("B", 7),
            ("C", 9),
            ("D", 11),
            ("E", 20),
            ("F", 20)
        ]
    );

    let scores = dijkstra(&g, a, Some(c), |e| *e.weight());
    assert_eq!(scores[&c], 9);
}

#[test]
fn test_astar_null_heuristic() {
    let mut g = Graph::new();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    let f = g.add_node("F");
    g.add_edge(a, b, 7);
    g.add_edge(c, a, 9);
    g.add_edge(a, d, 14);
    g.add_edge(b, c, 10);
    g.add_edge(d, c, 2);
    g.add_edge(d, e, 9);
    g.add_edge(b, f, 15);
    g.add_edge(c, f, 11);
    g.add_edge(e, f, 6);

    let path = astar(&g, a, |finish| finish == e, |e| *e.weight(), |_| 0);
    assert_eq!(path, Some((23, vec![a, d, e])));

    // check against dijkstra
    let dijkstra_run = dijkstra(&g, a, Some(e), |e| *e.weight());
    assert_eq!(dijkstra_run[&e], 23);

    let path = astar(&g, e, |finish| finish == b, |e| *e.weight(), |_| 0);
    assert_eq!(path, None);
}

#[test]
fn test_astar_manhattan_heuristic() {
    let mut g = Graph::new();
    let a = g.add_node((0., 0.));
    let b = g.add_node((2., 0.));
    let c = g.add_node((1., 1.));
    let d = g.add_node((0., 2.));
    let e = g.add_node((3., 3.));
    let f = g.add_node((4., 2.));
    let _ = g.add_node((5., 5.)); // no path to node
    g.add_edge(a, b, 2.);
    g.add_edge(a, d, 4.);
    g.add_edge(b, c, 1.);
    g.add_edge(b, f, 7.);
    g.add_edge(c, e, 5.);
    g.add_edge(e, f, 1.);
    g.add_edge(d, e, 1.);

    let heuristic_for = |f: NodeIndex| {
        let g = &g;
        move |node: NodeIndex| -> f32 {
            let (x1, y1): (f32, f32) = g[node];
            let (x2, y2): (f32, f32) = g[f];

            (x2 - x1).abs() + (y2 - y1).abs()
        }
    };
    let path = astar(
        &g,
        a,
        |finish| finish == f,
        |e| *e.weight(),
        heuristic_for(f),
    );

    assert_eq!(path, Some((6., vec![a, d, e, f])));

    // check against dijkstra
    let dijkstra_run = dijkstra(&g, a, None, |e| *e.weight());

    for end in g.node_indices() {
        let astar_path = astar(
            &g,
            a,
            |finish| finish == end,
            |e| *e.weight(),
            heuristic_for(end),
        );
        assert_eq!(dijkstra_run.get(&end).cloned(), astar_path.map(|t| t.0));
    }
}

#[test]
fn test_astar_runtime_optimal() {
    let mut g = Graph::new();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    g.add_edge(a, b, 2);
    g.add_edge(a, c, 3);
    g.add_edge(b, d, 3);
    g.add_edge(c, d, 1);
    g.add_edge(d, e, 1);

    let mut times_called = 0;

    let _ = astar(
        &g,
        a,
        |n| n == e,
        |edge| {
            times_called += 1;
            *edge.weight()
        },
        |_| 0,
    );

    // A* is runtime optimal in the sense it won't expand more nodes than needed, for the given
    // heuristic. Here, A* should expand, in order: A, B, C, D, E. This should should ask for the
    // costs of edges (A, B), (A, C), (B, D), (C, D), (D, E). Node D will be added to `visit_next`
    // twice, but should only be expanded once. If it is erroneously expanded twice, it will call
    // for (D, E) again and `times_called` will be 6.
    assert_eq!(times_called, 5);
}

#[test]
fn test_astar_admissible_inconsistent() {
    let mut g = Graph::new();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    g.add_edge(a, b, 3);
    g.add_edge(b, c, 3);
    g.add_edge(c, d, 3);
    g.add_edge(a, c, 8);
    g.add_edge(a, d, 10);

    let admissible_inconsistent = |n: NodeIndex| match g[n] {
        "A" => 9,
        "B" => 6,
        "C" => 0,
        &_ => 0,
    };

    let optimal = astar(&g, a, |n| n == d, |e| *e.weight(), admissible_inconsistent);
    assert_eq!(optimal, Some((9, vec![a, b, c, d])));
}

#[cfg(feature = "generate")]
#[test]
fn test_generate_undirected() {
    for size in 0..4 {
        let mut gen = pg::generate::Generator::<Undirected>::all(size, true);
        let nedges = (size * size - size) / 2 + size;
        let mut n = 0;
        while let Some(g) = gen.next_ref() {
            n += 1;
            println!("{:?}", g);
        }

        assert_eq!(n, 1 << nedges);
    }
}

#[cfg(feature = "generate")]
#[test]
fn test_generate_directed() {
    // Number of DAG out of all graphs (all permutations) per node size
    //            0, 1, 2, 3,  4,   5 ..
    let n_dag = &[1, 1, 3, 25, 543, 29281, 3781503];
    for (size, &dags_exp) in (0..4).zip(n_dag) {
        let mut gen = pg::generate::Generator::<Directed>::all(size, true);
        let nedges = size * size;
        let mut n = 0;
        let mut dags = 0;
        while let Some(g) = gen.next_ref() {
            n += 1;
            if !pg::algo::is_cyclic_directed(g) {
                dags += 1;
            }
        }

        /*
        // check that all generated graphs have unique adjacency matrices
        let mut adjmats = graphs.iter().map(Graph::adjacency_matrix).collect::<Vec<_>>();
        adjmats.sort(); adjmats.dedup();
        assert_eq!(adjmats.len(), n);
        */
        assert_eq!(dags_exp, dags);
        assert_eq!(n, 1 << nedges);
    }
}

#[cfg(feature = "generate")]
#[test]
fn test_generate_dag() {
    use petgraph::visit::GetAdjacencyMatrix;
    for size in 1..5 {
        let gen = pg::generate::Generator::directed_acyclic(size);
        let nedges = (size - 1) * size / 2;
        let graphs = gen.collect::<Vec<_>>();

        assert_eq!(graphs.len(), 1 << nedges);

        // check that all generated graphs have unique adjacency matrices
        let mut adjmats = graphs
            .iter()
            .map(Graph::adjacency_matrix)
            .collect::<Vec<_>>();
        adjmats.sort();
        adjmats.dedup();
        assert_eq!(adjmats.len(), graphs.len());
        for gr in &graphs {
            assert!(
                !petgraph::algo::is_cyclic_directed(gr),
                "Assertion failed: {:?} acyclic",
                gr
            );
        }
    }
}

#[test]
fn without() {
    let mut og = Graph::new_undirected();
    let a = og.add_node(0);
    let b = og.add_node(1);
    let c = og.add_node(2);
    let d = og.add_node(3);
    let _ = og.add_edge(a, b, 0);
    let _ = og.add_edge(a, c, 1);
    let v: Vec<NodeIndex> = og.externals(Outgoing).collect();
    assert_eq!(v, vec![d]);

    let mut og = Graph::new();
    let a = og.add_node(0);
    let b = og.add_node(1);
    let c = og.add_node(2);
    let d = og.add_node(3);
    let _ = og.add_edge(a, b, 0);
    let _ = og.add_edge(a, c, 1);
    let init: Vec<NodeIndex> = og.externals(Incoming).collect();
    let term: Vec<NodeIndex> = og.externals(Outgoing).collect();
    assert_eq!(init, vec![a, d]);
    assert_eq!(term, vec![b, c, d]);
}

fn assert_is_topo_order<N, E>(gr: &Graph<N, E, Directed>, order: &[NodeIndex]) {
    assert_eq!(gr.node_count(), order.len());
    // check all the edges of the graph
    for edge in gr.raw_edges() {
        let a = edge.source();
        let b = edge.target();
        let ai = order.iter().position(|x| *x == a).unwrap();
        let bi = order.iter().position(|x| *x == b).unwrap();
        println!("Check that {:?} is before {:?}", a, b);
        assert!(
            ai < bi,
            "Topo order: assertion that node {:?} is before {:?} failed",
            a,
            b
        );
    }
}

#[test]
fn test_toposort() {
    let mut gr = Graph::<_, _>::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let f = gr.add_node("F");
    let g = gr.add_node("G");
    gr.extend_with_edges(&[
        (a, b, 7.),
        (a, d, 5.),
        (d, b, 9.),
        (b, c, 8.),
        (b, e, 7.),
        (c, e, 5.),
        (d, e, 15.),
        (d, f, 6.),
        (f, e, 8.),
        (f, g, 11.),
        (e, g, 9.),
    ]);

    // add a disjoint part
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);

    let order = petgraph::algo::toposort(&gr, None).unwrap();
    println!("{:?}", order);
    assert_eq!(order.len(), gr.node_count());

    assert_is_topo_order(&gr, &order);
}

#[test]
fn test_toposort_eq() {
    let mut g = Graph::<_, _>::new();
    let a = g.add_node("A");
    let b = g.add_node("B");
    g.add_edge(a, b, ());

    assert_eq!(petgraph::algo::toposort(&g, None), Ok(vec![a, b]));
}

#[test]
fn is_cyclic_directed() {
    let mut gr = Graph::<_, _>::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let f = gr.add_node("F");
    let g = gr.add_node("G");
    gr.add_edge(a, b, 7.0);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);

    assert!(!petgraph::algo::is_cyclic_directed(&gr));

    // add a disjoint part
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);
    assert!(!petgraph::algo::is_cyclic_directed(&gr));

    gr.add_edge(g, e, 0.);
    assert!(petgraph::algo::is_cyclic_directed(&gr));
}

/// Compare two scc sets. Inside each scc, the order does not matter,
/// but the order of the sccs is significant.
fn assert_sccs_eq(
    mut res: Vec<Vec<NodeIndex>>,
    mut answer: Vec<Vec<NodeIndex>>,
    scc_order_matters: bool,
) {
    // normalize the result and compare with the answer.
    for scc in &mut res {
        scc.sort();
    }
    for scc in &mut answer {
        scc.sort();
    }
    if !scc_order_matters {
        res.sort();
        answer.sort();
    }
    assert_eq!(res, answer);
}

#[test]
fn scc() {
    let gr: Graph<(), ()> = Graph::from_edges(&[
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

    assert_sccs_eq(
        petgraph::algo::kosaraju_scc(&gr),
        vec![
            vec![n(0), n(3), n(6)],
            vec![n(2), n(5), n(8)],
            vec![n(1), n(4), n(7)],
        ],
        true,
    );
    // Reversed edges gives the same sccs (when sorted)
    assert_sccs_eq(
        petgraph::algo::kosaraju_scc(Reversed(&gr)),
        vec![
            vec![n(1), n(4), n(7)],
            vec![n(2), n(5), n(8)],
            vec![n(0), n(3), n(6)],
        ],
        true,
    );

    // Test an undirected graph just for fun.
    // Sccs are just connected components.
    let mut hr = gr.into_edge_type::<Undirected>();
    // Delete an edge to disconnect it
    let ed = hr.find_edge(n(6), n(8)).unwrap();
    assert!(hr.remove_edge(ed).is_some());

    assert_sccs_eq(
        petgraph::algo::kosaraju_scc(&hr),
        vec![
            vec![n(0), n(3), n(6)],
            vec![n(1), n(2), n(4), n(5), n(7), n(8)],
        ],
        false,
    );

    // acyclic non-tree, #14
    let n = NodeIndex::new;
    let mut gr = Graph::new();
    gr.add_node(0);
    gr.add_node(1);
    gr.add_node(2);
    gr.add_node(3);
    gr.add_edge(n(3), n(2), ());
    gr.add_edge(n(3), n(1), ());
    gr.add_edge(n(2), n(0), ());
    gr.add_edge(n(1), n(0), ());

    assert_sccs_eq(
        petgraph::algo::kosaraju_scc(&gr),
        vec![vec![n(0)], vec![n(1)], vec![n(2)], vec![n(3)]],
        true,
    );

    // Kosaraju bug from PR #60
    let mut gr = Graph::<(), ()>::new();
    gr.extend_with_edges(&[(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)]);
    gr.add_node(());
    // no order for the disconnected one
    assert_sccs_eq(
        petgraph::algo::kosaraju_scc(&gr),
        vec![vec![n(0)], vec![n(1)], vec![n(2)], vec![n(3)]],
        false,
    );
}

#[test]
fn tarjan_scc() {
    let gr: Graph<(), ()> = Graph::from_edges(&[
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

    let mut tarjan_scc = petgraph::algo::TarjanScc::new();

    let mut result = Vec::new();
    tarjan_scc.run(&gr, |scc| result.push(scc.iter().rev().cloned().collect()));
    assert_sccs_eq(
        result,
        vec![
            vec![n(0), n(3), n(6)],
            vec![n(2), n(5), n(8)],
            vec![n(1), n(4), n(7)],
        ],
        true,
    );

    // Test an undirected graph just for fun.
    // Sccs are just connected components.
    let mut hr = gr.into_edge_type::<Undirected>();
    // Delete an edge to disconnect it
    let ed = hr.find_edge(n(6), n(8)).unwrap();
    assert!(hr.remove_edge(ed).is_some());

    let mut result = Vec::new();
    tarjan_scc.run(&hr, |scc| result.push(scc.iter().rev().cloned().collect()));
    assert_sccs_eq(
        result,
        vec![
            vec![n(1), n(2), n(4), n(5), n(7), n(8)],
            vec![n(0), n(3), n(6)],
        ],
        false,
    );

    // acyclic non-tree, #14
    let n = NodeIndex::new;
    let mut gr = Graph::new();
    gr.add_node(0);
    gr.add_node(1);
    gr.add_node(2);
    gr.add_node(3);
    gr.add_edge(n(3), n(2), ());
    gr.add_edge(n(3), n(1), ());
    gr.add_edge(n(2), n(0), ());
    gr.add_edge(n(1), n(0), ());

    let mut result = Vec::new();
    tarjan_scc.run(&gr, |scc| result.push(scc.iter().rev().cloned().collect()));
    assert_sccs_eq(
        result,
        vec![vec![n(0)], vec![n(1)], vec![n(2)], vec![n(3)]],
        true,
    );

    // Kosaraju bug from PR #60
    let mut gr = Graph::<(), ()>::new();
    gr.extend_with_edges(&[(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)]);
    gr.add_node(());
    // no order for the disconnected one
    let mut result = Vec::new();
    tarjan_scc.run(&gr, |scc| result.push(scc.iter().rev().cloned().collect()));
    assert_sccs_eq(
        result,
        vec![vec![n(0)], vec![n(1)], vec![n(2)], vec![n(3)]],
        false,
    );
}

#[test]
fn condensation() {
    let gr: Graph<(), ()> = Graph::from_edges(&[
        (6, 0),
        (0, 3),
        (3, 6),
        (8, 6),
        (8, 2),
        (2, 3),
        (2, 5),
        (5, 8),
        (7, 5),
        (1, 7),
        (7, 4),
        (4, 1),
    ]);

    // make_acyclic = true

    let cond = petgraph::algo::condensation(gr.clone(), true);

    assert!(cond.node_count() == 3);
    assert!(cond.edge_count() == 2);
    assert!(
        !petgraph::algo::is_cyclic_directed(&cond),
        "Assertion failed: {:?} acyclic",
        cond
    );

    // make_acyclic = false

    let cond = petgraph::algo::condensation(gr.clone(), false);

    assert!(cond.node_count() == 3);
    assert!(cond.edge_count() == gr.edge_count());
}

#[test]
fn connected_comp() {
    let n = NodeIndex::new;
    let mut gr = Graph::new();
    gr.add_node(0);
    gr.add_node(1);
    gr.add_node(2);
    gr.add_node(3);
    gr.add_node(4);
    gr.add_node(5);
    gr.add_node(6);
    gr.add_node(7);
    gr.add_node(8);
    gr.add_edge(n(6), n(0), ());
    gr.add_edge(n(0), n(3), ());
    gr.add_edge(n(3), n(6), ());
    gr.add_edge(n(8), n(6), ());
    gr.add_edge(n(8), n(2), ());
    gr.add_edge(n(2), n(5), ());
    gr.add_edge(n(5), n(8), ());
    gr.add_edge(n(7), n(5), ());
    gr.add_edge(n(1), n(7), ());
    gr.add_edge(n(7), n(4), ());
    gr.add_edge(n(4), n(1), ());
    assert_eq!(petgraph::algo::connected_components(&gr), 1);

    gr.add_node(9);
    gr.add_node(10);
    assert_eq!(petgraph::algo::connected_components(&gr), 3);

    gr.add_edge(n(9), n(10), ());
    assert_eq!(petgraph::algo::connected_components(&gr), 2);

    let gr = gr.into_edge_type::<Undirected>();
    assert_eq!(petgraph::algo::connected_components(&gr), 2);
}

#[should_panic]
#[test]
fn oob_index() {
    let mut gr = Graph::<_, ()>::new();
    let a = gr.add_node(0);
    let b = gr.add_node(1);
    gr.remove_node(a);
    gr[b];
}

#[test]
fn usize_index() {
    let mut gr = Graph::<_, _, Directed, usize>::with_capacity(0, 0);
    let a = gr.add_node(0);
    let b = gr.add_node(1);
    let e = gr.add_edge(a, b, 1.2);
    let mut dfs = Dfs::new(&gr, a);
    while let Some(nx) = dfs.next(&gr) {
        gr[nx] += 1;
    }
    assert_eq!(gr[a], 1);
    assert_eq!(gr[b], 2);
    assert_eq!(gr[e], 1.2);
}

#[test]
fn u8_index() {
    let mut gr = Graph::<_, (), Undirected, u8>::with_capacity(0, 0);
    for _ in 0..255 {
        gr.add_node(());
    }
}

#[should_panic]
#[test]
fn u8_index_overflow() {
    let mut gr = Graph::<_, (), Undirected, u8>::with_capacity(0, 0);
    for _ in 0..256 {
        gr.add_node(());
    }
}

#[should_panic]
#[test]
fn u8_index_overflow_edges() {
    let mut gr = Graph::<_, (), Undirected, u8>::with_capacity(0, 0);
    let a = gr.add_node('a');
    let b = gr.add_node('b');
    for _ in 0..256 {
        gr.add_edge(a, b, ());
    }
}

#[test]
fn test_weight_iterators() {
    let mut gr = Graph::<_, _>::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let f = gr.add_node("F");
    let g = gr.add_node("G");
    gr.add_edge(a, b, 7.0);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);

    assert_eq!(gr.node_weights_mut().count(), gr.node_count());
    assert_eq!(gr.edge_weights_mut().count(), gr.edge_count());

    // add a disjoint part
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);

    assert_eq!(gr.node_weights_mut().count(), gr.node_count());
    assert_eq!(gr.edge_weights_mut().count(), gr.edge_count());

    for nw in gr.node_weights_mut() {
        *nw = "x";
    }
    for node in gr.raw_nodes() {
        assert_eq!(node.weight, "x");
    }

    let old = gr.clone();
    for (index, ew) in gr.edge_weights_mut().enumerate() {
        assert_eq!(old[EdgeIndex::new(index)], *ew);
        *ew = -*ew;
    }
    for (index, edge) in gr.raw_edges().iter().enumerate() {
        assert_eq!(edge.weight, -1. * old[EdgeIndex::new(index)]);
    }
}

#[test]
fn walk_edges() {
    let mut gr = Graph::<_, _>::new();
    let a = gr.add_node(0.);
    let b = gr.add_node(1.);
    let c = gr.add_node(2.);
    let d = gr.add_node(3.);
    let e0 = gr.add_edge(a, b, 0.);
    let e1 = gr.add_edge(a, d, 0.);
    let e2 = gr.add_edge(b, c, 0.);
    let e3 = gr.add_edge(c, a, 0.);

    // Set edge weights to difference: target - source.
    let mut dfs = Dfs::new(&gr, a);
    while let Some(source) = dfs.next(&gr) {
        let mut edges = gr.neighbors_directed(source, Outgoing).detach();
        while let Some((edge, target)) = edges.next(&gr) {
            gr[edge] = gr[target] - gr[source];
        }
    }
    assert_eq!(gr[e0], 1.);
    assert_eq!(gr[e1], 3.);
    assert_eq!(gr[e2], 1.);
    assert_eq!(gr[e3], -2.);

    let mut nedges = 0;
    let mut dfs = Dfs::new(&gr, a);
    while let Some(node) = dfs.next(&gr) {
        let mut edges = gr.neighbors_directed(node, Incoming).detach();
        while let Some((edge, source)) = edges.next(&gr) {
            assert_eq!(gr.find_edge(source, node), Some(edge));
            nedges += 1;
        }

        let mut edges = gr.neighbors_directed(node, Outgoing).detach();
        while let Some((edge, target)) = edges.next(&gr) {
            assert_eq!(gr.find_edge(node, target), Some(edge));
            nedges += 1;
        }
    }
    assert_eq!(nedges, 8);
}

#[test]
fn index_twice_mut() {
    let mut gr = Graph::<_, _>::new();
    let a = gr.add_node(0.);
    let b = gr.add_node(0.);
    let c = gr.add_node(0.);
    let d = gr.add_node(0.);
    let e = gr.add_node(0.);
    let f = gr.add_node(0.);
    let g = gr.add_node(0.);
    gr.add_edge(a, b, 7.0);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);

    for dir in &[Incoming, Outgoing] {
        for nw in gr.node_weights_mut() {
            *nw = 0.;
        }

        // walk the graph and sum incoming edges
        let mut dfs = Dfs::new(&gr, a);
        while let Some(node) = dfs.next(&gr) {
            let mut edges = gr.neighbors_directed(node, *dir).detach();
            while let Some(edge) = edges.next_edge(&gr) {
                let (nw, ew) = gr.index_twice_mut(node, edge);
                *nw += *ew;
            }
        }

        // check the sums
        for i in 0..gr.node_count() {
            let ni = NodeIndex::new(i);
            let s = gr
                .edges_directed(ni, *dir)
                .map(|e| *e.weight())
                .fold(0., |a, b| a + b);
            assert_eq!(s, gr[ni]);
        }
        println!("Sum {:?}: {:?}", dir, gr);
    }
}

fn make_edge_iterator_graph<Ty: EdgeType>() -> Graph<f64, f64, Ty> {
    let mut gr = Graph::default();
    let a = gr.add_node(0.);
    let b = gr.add_node(0.);
    let c = gr.add_node(0.);
    let d = gr.add_node(0.);
    let e = gr.add_node(0.);
    let f = gr.add_node(0.);
    let g = gr.add_node(0.);
    gr.add_edge(a, b, 7.0);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, c, 8.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);

    gr
}

#[test]
fn test_edge_iterators_directed() {
    let gr = make_edge_iterator_graph::<Directed>();

    for i in gr.node_indices() {
        itertools::assert_equal(gr.edges_directed(i, Outgoing), gr.edges(i));

        // Reversed reverses edges, so target and source need to be reversed once more.
        itertools::assert_equal(
            gr.edges_directed(i, Outgoing)
                .map(|edge| (edge.source(), edge.target())),
            Reversed(&gr)
                .edges_directed(i, Incoming)
                .map(|edge| (edge.target(), edge.source())),
        );

        for edge in gr.edges_directed(i, Outgoing) {
            assert_eq!(
                edge.source(),
                i,
                "outgoing edges should have a fixed source"
            );
        }
        for edge in Reversed(&gr).edges_directed(i, Incoming) {
            assert_eq!(
                edge.target(),
                i,
                "reversed incoming edges should have a fixed target"
            );
        }
    }

    let mut reversed_gr = gr.clone();
    reversed_gr.reverse();

    println!("{:#?}", gr);
    for i in gr.node_indices() {
        // Compare against reversed graphs two different ways: using .reverse() and Reversed.
        itertools::assert_equal(gr.edges_directed(i, Incoming), reversed_gr.edges(i));

        // Reversed reverses edges, so target and source need to be reversed once more.
        itertools::assert_equal(
            gr.edges_directed(i, Incoming)
                .map(|edge| (edge.source(), edge.target())),
            Reversed(&gr)
                .edges(i)
                .map(|edge| (edge.target(), edge.source())),
        );

        for edge in gr.edges_directed(i, Incoming) {
            assert_eq!(
                edge.target(),
                i,
                "incoming edges should have a fixed target"
            );
        }
        for edge in Reversed(&gr).edges_directed(i, Outgoing) {
            assert_eq!(
                edge.source(),
                i,
                "reversed outgoing edges should have a fixed source"
            );
        }
    }
}

#[test]
fn test_edge_filtered_iterators_directed() {
    use petgraph::{
        graph::EdgeReference,
        visit::{EdgeFiltered, IntoEdgesDirected},
    };

    let gr = make_edge_iterator_graph::<Directed>();
    let filter = |edge: EdgeReference<f64>| -> bool { *edge.weight() > 8.0 };
    let filtered = EdgeFiltered::from_fn(&gr, filter);

    for i in gr.node_indices() {
        itertools::assert_equal(
            filtered.edges_directed(i, Outgoing),
            gr.edges_directed(i, Outgoing).filter(|edge| filter(*edge)),
        );
        itertools::assert_equal(
            filtered.edges_directed(i, Incoming),
            gr.edges_directed(i, Incoming).filter(|edge| filter(*edge)),
        );
    }
}

#[test]
fn test_node_filtered_iterators_directed() {
    use petgraph::{
        graph::NodeIndex,
        visit::{IntoEdgesDirected, NodeFiltered},
    };

    let gr = make_edge_iterator_graph::<Directed>();
    let filter = |node: NodeIndex<u32>| node.index() < 5; // < 5 makes sure there are edges going both ways between included and excluded nodes (e -> g, f -> e)
    let filtered = NodeFiltered::from_fn(&gr, filter);

    for i in gr.node_indices() {
        itertools::assert_equal(
            filtered.edges_directed(i, Outgoing),
            gr.edges_directed(i, Outgoing)
                .filter(|edge| filter(edge.source()) && filter(edge.target())),
        );
        itertools::assert_equal(
            filtered.edges_directed(i, Incoming),
            gr.edges_directed(i, Incoming)
                .filter(|edge| filter(edge.source()) && filter(edge.target())),
        );
    }
}

#[test]
fn test_edge_iterators_undir() {
    let gr = make_edge_iterator_graph::<Undirected>();

    for i in gr.node_indices() {
        itertools::assert_equal(gr.edges_directed(i, Outgoing), gr.edges(i));

        // Reversed reverses edges, so target and source need to be reversed once more.
        itertools::assert_equal(
            gr.edges_directed(i, Outgoing)
                .map(|edge| (edge.source(), edge.target())),
            Reversed(&gr)
                .edges_directed(i, Incoming)
                .map(|edge| (edge.target(), edge.source())),
        );

        for edge in gr.edges_directed(i, Outgoing) {
            assert_eq!(
                edge.source(),
                i,
                "outgoing edges should have a fixed source"
            );
        }
        for edge in Reversed(&gr).edges_directed(i, Incoming) {
            assert_eq!(
                edge.target(),
                i,
                "reversed incoming edges should have a fixed target"
            );
        }
    }

    for i in gr.node_indices() {
        itertools::assert_equal(gr.edges_directed(i, Incoming), gr.edges(i));

        // Reversed reverses edges, so target and source need to be reversed once more.
        itertools::assert_equal(
            gr.edges_directed(i, Incoming)
                .map(|edge| (edge.source(), edge.target())),
            Reversed(&gr)
                .edges(i)
                .map(|edge| (edge.target(), edge.source())),
        );

        for edge in gr.edges_directed(i, Incoming) {
            assert_eq!(
                edge.target(),
                i,
                "incoming edges should have a fixed target"
            );
        }
        for edge in Reversed(&gr).edges_directed(i, Outgoing) {
            assert_eq!(
                edge.source(),
                i,
                "reversed outgoing edges should have a fixed source"
            );
        }
    }
}

#[test]
fn toposort_generic() {
    // This is a DAG, visit it in order
    let mut gr = Graph::<_, _>::new();
    let b = gr.add_node(("B", 0.));
    let a = gr.add_node(("A", 0.));
    let c = gr.add_node(("C", 0.));
    let d = gr.add_node(("D", 0.));
    let e = gr.add_node(("E", 0.));
    let f = gr.add_node(("F", 0.));
    let g = gr.add_node(("G", 0.));
    gr.add_edge(a, b, 7.0);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);

    assert!(!pg::algo::is_cyclic_directed(&gr));
    let mut index = 0.;
    let mut topo = Topo::new(&gr);
    while let Some(nx) = topo.next(&gr) {
        gr[nx].1 = index;
        index += 1.;
    }

    let mut order = Vec::new();
    index = 0.;
    let mut topo = Topo::new(&gr);
    while let Some(nx) = topo.next(&gr) {
        order.push(nx);
        assert_eq!(gr[nx].1, index);
        index += 1.;
    }
    println!("{:?}", gr);
    assert_is_topo_order(&gr, &order);

    {
        order.clear();
        let init_nodes = gr.node_identifiers().filter(|n| {
            gr.neighbors_directed(*n, Direction::Incoming)
                .next()
                .is_none()
        });
        let mut topo = Topo::with_initials(&gr, init_nodes);
        while let Some(nx) = topo.next(&gr) {
            order.push(nx);
        }
        assert_is_topo_order(&gr, &order);
    }

    {
        // test `with_initials` API using nodes with incoming edges
        order.clear();
        let mut topo = Topo::with_initials(&gr, gr.node_identifiers());
        while let Some(nx) = topo.next(&gr) {
            order.push(nx);
        }
        assert_is_topo_order(&gr, &order);
    }

    {
        order.clear();
        let mut topo = Topo::new(&gr);
        while let Some(nx) = topo.next(&gr) {
            order.push(nx);
        }
        println!("{:?}", gr);
        assert_is_topo_order(&gr, &order);
    }
    let mut gr2 = gr.clone();
    gr.add_edge(e, d, -1.);
    assert!(pg::algo::is_cyclic_directed(&gr));
    assert!(pg::algo::toposort(&gr, None).is_err());
    gr2.add_edge(d, d, 0.);
    assert!(pg::algo::is_cyclic_directed(&gr2));
    assert!(pg::algo::toposort(&gr2, None).is_err());
}

#[test]
fn test_has_path() {
    // This is a DAG, visit it in order
    let mut gr = Graph::<_, _>::new();
    let b = gr.add_node(("B", 0.));
    let a = gr.add_node(("A", 0.));
    let c = gr.add_node(("C", 0.));
    let d = gr.add_node(("D", 0.));
    let e = gr.add_node(("E", 0.));
    let f = gr.add_node(("F", 0.));
    let g = gr.add_node(("G", 0.));
    gr.add_edge(a, b, 7.0);
    gr.add_edge(a, d, 5.);
    gr.add_edge(d, b, 9.);
    gr.add_edge(b, c, 8.);
    gr.add_edge(b, e, 7.);
    gr.add_edge(c, e, 5.);
    gr.add_edge(d, e, 15.);
    gr.add_edge(d, f, 6.);
    gr.add_edge(f, e, 8.);
    gr.add_edge(f, g, 11.);
    gr.add_edge(e, g, 9.);
    // disconnected island

    let h = gr.add_node(("H", 0.));
    let i = gr.add_node(("I", 0.));
    gr.add_edge(h, i, 2.);
    gr.add_edge(i, h, -2.);

    let mut state = DfsSpace::default();

    gr.add_edge(b, a, 99.);

    assert!(has_path_connecting(&gr, c, c, None));

    for edge in gr.edge_references() {
        assert!(has_path_connecting(&gr, edge.source(), edge.target(), None));
    }
    assert!(has_path_connecting(&gr, a, g, Some(&mut state)));
    assert!(!has_path_connecting(&gr, a, h, Some(&mut state)));
    assert!(has_path_connecting(&gr, a, c, None));
    assert!(has_path_connecting(&gr, a, c, Some(&mut state)));
    assert!(!has_path_connecting(&gr, h, a, Some(&mut state)));
}

#[test]
fn map_filter_map() {
    let mut g = Graph::new_undirected();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    let f = g.add_node("F");
    g.add_edge(a, b, 7);
    g.add_edge(c, a, 9);
    g.add_edge(a, d, 14);
    g.add_edge(b, c, 10);
    g.add_edge(d, c, 2);
    g.add_edge(d, e, 9);
    g.add_edge(b, f, 15);
    g.add_edge(c, f, 11);
    g.add_edge(e, f, 6);
    println!("{:?}", g);

    let g2 = g.filter_map(
        |_, name| Some(*name),
        |_, &weight| if weight >= 10 { Some(weight) } else { None },
    );
    assert_eq!(g2.edge_count(), 4);
    for edge in g2.raw_edges() {
        assert!(edge.weight >= 10);
    }

    let g3 = g.filter_map(
        |i, &name| if i == a || i == e { None } else { Some(name) },
        |i, &weight| {
            let (source, target) = g.edge_endpoints(i).unwrap();
            // don't map edges from a removed node
            assert!(source != a);
            assert!(target != a);
            assert!(source != e);
            assert!(target != e);
            Some(weight)
        },
    );
    assert_eq!(g3.node_count(), g.node_count() - 2);
    assert_eq!(g3.edge_count(), g.edge_count() - 5);
    assert_graph_consistent(&g3);

    let mut g4 = g.clone();
    g4.retain_edges(|gr, i| {
        let (s, t) = gr.edge_endpoints(i).unwrap();
        !(s == a || s == e || t == a || t == e)
    });
    assert_eq!(g4.edge_count(), g.edge_count() - 5);
    assert_graph_consistent(&g4);
}

#[test]
fn from_edges() {
    let n = NodeIndex::new;
    let gr =
        Graph::<(), (), Undirected>::from_edges(&[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
    assert_eq!(gr.node_count(), 4);
    assert_eq!(gr.edge_count(), 6);
    assert_eq!(gr.neighbors(n(0)).count(), 3);
    assert_eq!(gr.neighbors(n(1)).count(), 3);
    assert_eq!(gr.neighbors(n(2)).count(), 3);
    assert_eq!(gr.neighbors(n(3)).count(), 3);
    assert_graph_consistent(&gr);
}

#[test]
fn retain() {
    let mut gr = Graph::<i32, i32, Undirected>::from_edges(&[
        (0, 1, 2),
        (1, 1, 1),
        (0, 2, 0),
        (1, 2, 3),
        (2, 3, 3),
    ]);
    gr.retain_edges(|mut gr, i| {
        if gr[i] <= 0 {
            false
        } else {
            gr[i] -= 1;
            let (s, t) = gr.edge_endpoints(i).unwrap();
            gr[s] += 1;
            gr[t] += 1;

            gr[i] > 0
        }
    });

    assert_eq!(gr.edge_count(), 3);
    assert_eq!(gr[n(0)], 1);
    assert_eq!(gr[n(1)], 4);
    assert_eq!(gr[n(2)], 2);
    assert_eq!(gr[n(3)], 1);
    assert!(gr.find_edge(n(1), n(1)).is_none());
    assert!(gr.find_edge(n(0), n(2)).is_none());
    assert_graph_consistent(&gr);
}

fn assert_graph_consistent<N, E, Ty, Ix>(g: &Graph<N, E, Ty, Ix>)
where
    Ty: EdgeType,
    Ix: IndexType,
{
    assert_eq!(g.node_count(), g.node_indices().count());
    assert_eq!(g.edge_count(), g.edge_indices().count());
    for edge in g.raw_edges() {
        assert!(
            g.find_edge(edge.source(), edge.target()).is_some(),
            "Edge not in graph! {:?} to {:?}",
            edge.source(),
            edge.target()
        );
    }
}

#[test]
fn neighbors_selfloops() {
    // Directed graph
    let mut gr = Graph::<_, ()>::new();
    let a = gr.add_node("a");
    let b = gr.add_node("b");
    let c = gr.add_node("c");
    gr.extend_with_edges(&[(a, a), (a, b), (c, a), (a, a)]);

    let out_edges = [a, a, b];
    let in_edges = [a, a, c];
    let undir_edges = [a, a, b, c];
    let mut seen_out = gr.neighbors(a).collect::<Vec<_>>();
    seen_out.sort();
    assert_eq!(&seen_out, &out_edges);
    let mut seen_in = gr.neighbors_directed(a, Incoming).collect::<Vec<_>>();
    seen_in.sort();
    assert_eq!(&seen_in, &in_edges);

    let mut seen_undir = gr.neighbors_undirected(a).collect::<Vec<_>>();
    seen_undir.sort();
    assert_eq!(&seen_undir, &undir_edges);

    let mut seen_out = gr.edges(a).map(|e| e.target()).collect::<Vec<_>>();
    seen_out.sort();
    assert_eq!(&seen_out, &out_edges);

    let mut seen_walk = Vec::new();
    let mut walk = gr.neighbors(a).detach();
    while let Some(n) = walk.next_node(&gr) {
        seen_walk.push(n);
    }
    seen_walk.sort();
    assert_eq!(&seen_walk, &out_edges);

    seen_walk.clear();
    let mut walk = gr.neighbors_directed(a, Incoming).detach();
    while let Some(n) = walk.next_node(&gr) {
        seen_walk.push(n);
    }
    seen_walk.sort();
    assert_eq!(&seen_walk, &in_edges);

    seen_walk.clear();
    let mut walk = gr.neighbors_undirected(a).detach();
    while let Some(n) = walk.next_node(&gr) {
        seen_walk.push(n);
    }
    seen_walk.sort();
    assert_eq!(&seen_walk, &undir_edges);

    // Undirected graph
    let mut gr: Graph<_, (), _> = Graph::new_undirected();
    let a = gr.add_node("a");
    let b = gr.add_node("b");
    let c = gr.add_node("c");
    gr.extend_with_edges(&[(a, a), (a, b), (c, a)]);

    let out_edges = [a, b, c];
    let in_edges = [a, b, c];
    let undir_edges = [a, b, c];
    let mut seen_out = gr.neighbors(a).collect::<Vec<_>>();
    seen_out.sort();
    assert_eq!(&seen_out, &out_edges);

    let mut seen_out = gr.edges(a).map(|e| e.target()).collect::<Vec<_>>();
    seen_out.sort();
    assert_eq!(&seen_out, &out_edges);

    let mut seen_in = gr.neighbors_directed(a, Incoming).collect::<Vec<_>>();
    seen_in.sort();
    assert_eq!(&seen_in, &in_edges);

    let mut seen_undir = gr.neighbors_undirected(a).collect::<Vec<_>>();
    seen_undir.sort();
    assert_eq!(&seen_undir, &undir_edges);
}

fn degree<'a, G>(g: G, node: G::NodeId) -> usize
where
    G: IntoNeighbors,
    G::NodeId: PartialEq,
{
    // self loops count twice
    let original_node = node;
    let mut degree = 0;
    for v in g.neighbors(node) {
        degree += if v == original_node { 2 } else { 1 };
    }
    degree
}

#[cfg(feature = "graphmap")]
#[test]
fn degree_sequence() {
    let mut gr = Graph::<usize, (), Undirected>::from_edges(&[
        (0, 1),
        (1, 2),
        (1, 3),
        (2, 4),
        (3, 4),
        (4, 4),
        (4, 5),
        (3, 5),
    ]);
    gr.add_node(0); // add isolated node
    let mut degree_sequence = gr
        .node_indices()
        .map(|i| degree(&gr, i))
        .collect::<Vec<_>>();

    degree_sequence.sort_by(|x, y| Ord::cmp(y, x));
    assert_eq!(&degree_sequence, &[5, 3, 3, 2, 2, 1, 0]);

    let mut gr = GraphMap::<_, (), Undirected>::from_edges(&[
        (0, 1),
        (1, 2),
        (1, 3),
        (2, 4),
        (3, 4),
        (4, 4),
        (4, 5),
        (3, 5),
    ]);
    gr.add_node(6); // add isolated node
    let mut degree_sequence = gr.nodes().map(|i| degree(&gr, i)).collect::<Vec<_>>();

    degree_sequence.sort_by(|x, y| Ord::cmp(y, x));
    assert_eq!(&degree_sequence, &[5, 3, 3, 2, 2, 1, 0]);
}

#[test]
fn neighbor_order() {
    let mut gr = Graph::new();
    let a = gr.add_node("a");
    let b = gr.add_node("b");
    let c = gr.add_node("c");
    gr.add_edge(a, b, 0);
    gr.add_edge(a, a, 1);

    gr.add_edge(c, a, 2);

    gr.add_edge(a, c, 3);

    gr.add_edge(c, a, 4);
    gr.add_edge(b, a, 5);

    // neighbors (edges) are in lifo order, if it's a directed graph
    assert_eq!(gr.neighbors(a).collect::<Vec<_>>(), vec![c, a, b]);
    assert_eq!(
        gr.neighbors_directed(a, Incoming).collect::<Vec<_>>(),
        vec![b, c, c, a]
    );
}

#[test]
fn dot() {
    // test alternate formatting
    #[derive(Debug)]
    struct Record {
        a: i32,
        b: &'static str,
    }
    let mut gr = Graph::new();
    let a = gr.add_node(Record { a: 1, b: r"abc\" });
    gr.add_edge(a, a, (1, 2));
    let dot_output = format!("{:?}", Dot::new(&gr));
    assert_eq!(
        dot_output,
        // The single \ turns into four \\\\ because of Debug which turns it to \\ and then escaping each \ to \\.
        r#"digraph {
    0 [ label = "Record { a: 1, b: \"abc\\\\\" }" ]
    0 -> 0 [ label = "(1, 2)" ]
}
"#
    );
}

#[test]
fn filtered() {
    let mut g = Graph::new();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    let f = g.add_node("F");
    g.add_edge(a, b, 7);
    g.add_edge(c, a, 9);
    g.add_edge(a, d, 14);
    g.add_edge(b, c, 10);
    g.add_edge(d, c, 2);
    g.add_edge(d, e, 9);
    g.add_edge(b, f, 15);
    g.add_edge(c, f, 11);
    g.add_edge(e, f, 6);
    println!("{:?}", g);

    let filt = NodeFiltered(&g, |n: NodeIndex| n != c && n != e);

    let mut dfs = DfsPostOrder::new(&filt, a);
    let mut po = Vec::new();
    while let Some(nx) = dfs.next(&filt) {
        println!("Next: {:?}", nx);
        po.push(nx);
    }
    assert_eq!(set(po), set(g.node_identifiers().filter(|n| (filt.1)(*n))));
}

#[test]
fn filtered_edge_reverse() {
    use petgraph::visit::EdgeFiltered;
    #[derive(Eq, PartialEq)]
    enum E {
        A,
        B,
    }

    // Start with single node graph with loop
    let mut g = Graph::new();
    let a = g.add_node("A");
    g.add_edge(a, a, E::A);
    let ef_a = EdgeFiltered::from_fn(&g, |edge| *edge.weight() == E::A);
    let mut po = Vec::new();
    let mut dfs = Dfs::new(&ef_a, a);
    while let Some(next_n_ix) = dfs.next(&ef_a) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![a]));

    // Check in reverse
    let mut po = Vec::new();
    let mut dfs = Dfs::new(&Reversed(&ef_a), a);
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_a)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![a]));

    let mut g = Graph::new();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    let f = g.add_node("F");
    let h = g.add_node("H");
    let i = g.add_node("I");
    let j = g.add_node("J");

    g.add_edge(a, b, E::A);
    g.add_edge(b, c, E::A);
    g.add_edge(c, d, E::B);
    g.add_edge(d, e, E::A);
    g.add_edge(e, f, E::A);
    g.add_edge(e, h, E::A);
    g.add_edge(e, i, E::A);
    g.add_edge(i, j, E::A);

    let ef_a = EdgeFiltered::from_fn(&g, |edge| *edge.weight() == E::A);
    let ef_b = EdgeFiltered::from_fn(&g, |edge| *edge.weight() == E::B);

    // DFS down from a, filtered by E::A.
    let mut po = Vec::new();
    let mut dfs = Dfs::new(&ef_a, a);
    while let Some(next_n_ix) = dfs.next(&ef_a) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![a, b, c]));

    // Reversed DFS from f, filtered by E::A.
    let mut dfs = Dfs::new(&Reversed(&ef_a), f);
    let mut po = Vec::new();
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_a)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![d, e, f]));

    // Reversed DFS from j, filtered by E::A.
    let mut dfs = Dfs::new(&Reversed(&ef_a), j);
    let mut po = Vec::new();
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_a)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![d, e, i, j]));

    // Reversed DFS from c, filtered by E::A.
    let mut dfs = Dfs::new(&Reversed(&ef_a), c);
    let mut po = Vec::new();
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_a)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![a, b, c]));

    // Reversed DFS from c, filtered by E::B.
    let mut dfs = Dfs::new(&Reversed(&ef_b), c);
    let mut po = Vec::new();
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_b)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![c]));

    // Reversed DFS from d, filtered by E::B.
    let mut dfs = Dfs::new(&Reversed(&ef_b), d);
    let mut po = Vec::new();
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_b)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![c, d]));

    // Now let's test the same graph but undirected

    let mut g = Graph::new_undirected();
    let a = g.add_node("A");
    let b = g.add_node("B");
    let c = g.add_node("C");
    let d = g.add_node("D");
    let e = g.add_node("E");
    let f = g.add_node("F");
    let h = g.add_node("H");
    let i = g.add_node("I");
    let j = g.add_node("J");

    g.add_edge(a, b, E::A);
    g.add_edge(b, c, E::A);
    g.add_edge(c, d, E::B);
    g.add_edge(d, e, E::A);
    g.add_edge(e, f, E::A);
    g.add_edge(e, h, E::A);
    g.add_edge(e, i, E::A);
    g.add_edge(i, j, E::A);

    let ef_a = EdgeFiltered::from_fn(&g, |edge| *edge.weight() == E::A);
    let ef_b = EdgeFiltered::from_fn(&g, |edge| *edge.weight() == E::B);
    let mut po = Vec::new();
    let mut dfs = Dfs::new(&Reversed(&ef_b), d);
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_b)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![c, d]));

    let mut po = Vec::new();
    let mut dfs = Dfs::new(&Reversed(&ef_a), h);
    while let Some(next_n_ix) = dfs.next(&Reversed(&ef_a)) {
        po.push(next_n_ix);
    }
    assert_eq!(set(po), set(vec![d, e, f, h, i, j]));
}

#[test]
fn dfs_visit() {
    use petgraph::visit::Control;
    use petgraph::visit::DfsEvent::*;
    use petgraph::visit::{depth_first_search, Time};
    use petgraph::visit::{VisitMap, Visitable};
    let gr: Graph<(), ()> = Graph::from_edges(&[
        (0, 5),
        (0, 2),
        (0, 3),
        (0, 1),
        (1, 3),
        (2, 3),
        (2, 4),
        (4, 0),
        (4, 5),
    ]);

    let invalid_time = Time(!0);
    let mut discover_time = vec![invalid_time; gr.node_count()];
    let mut finish_time = vec![invalid_time; gr.node_count()];
    let mut has_tree_edge = gr.visit_map();
    let mut edges = HashSet::new();
    depth_first_search(&gr, Some(n(0)), |evt| {
        println!("Event: {:?}", evt);
        match evt {
            Discover(n, t) => discover_time[n.index()] = t,
            Finish(n, t) => finish_time[n.index()] = t,
            TreeEdge(u, v) => {
                // v is an ancestor of u
                assert!(has_tree_edge.visit(v), "Two tree edges to {:?}!", v);
                assert!(discover_time[v.index()] == invalid_time);
                assert!(discover_time[u.index()] != invalid_time);
                assert!(finish_time[u.index()] == invalid_time);
                edges.insert((u, v));
            }
            BackEdge(u, v) => {
                // u is an ancestor of v
                assert!(discover_time[v.index()] != invalid_time);
                assert!(finish_time[v.index()] == invalid_time);
                edges.insert((u, v));
            }
            CrossForwardEdge(u, v) => {
                edges.insert((u, v));
            }
        }
    });
    assert!(discover_time.iter().all(|x| *x != invalid_time));
    assert!(finish_time.iter().all(|x| *x != invalid_time));
    assert_eq!(edges.len(), gr.edge_count());
    assert_eq!(
        edges,
        set(gr.edge_references().map(|e| (e.source(), e.target())))
    );
    println!("{:?}", discover_time);
    println!("{:?}", finish_time);

    // find path from 0 to 4
    let mut predecessor = vec![NodeIndex::end(); gr.node_count()];
    let start = n(0);
    let goal = n(4);
    let ret = depth_first_search(&gr, Some(start), |event| {
        if let TreeEdge(u, v) = event {
            predecessor[v.index()] = u;
            if v == goal {
                return Control::Break(u);
            }
        }
        Control::Continue
    });
    // assert we did terminate early
    assert!(ret.break_value().is_some());
    assert!(predecessor.iter().any(|x| *x == NodeIndex::end()));

    let mut next = goal;
    let mut path = vec![next];
    while next != start {
        let pred = predecessor[next.index()];
        path.push(pred);
        next = pred;
    }
    path.reverse();
    assert_eq!(&path, &[n(0), n(2), n(4)]);

    // check that if we prune 2, we never see 4.
    let start = n(0);
    let prune = n(2);
    let nongoal = n(4);
    let ret = depth_first_search(&gr, Some(start), |event| {
        if let Discover(n, _) = event {
            if n == prune {
                return Control::Prune;
            }
        } else if let TreeEdge(u, v) = event {
            if v == nongoal {
                return Control::Break(u);
            }
        }
        Control::Continue
    });
    assert!(ret.break_value().is_none());
}

#[test]
fn filtered_post_order() {
    use petgraph::visit::NodeFiltered;

    let mut gr: Graph<(), ()> =
        Graph::from_edges(&[(0, 2), (1, 2), (0, 3), (1, 4), (2, 4), (4, 5), (3, 5)]);
    // map reachable nodes
    let mut dfs = Dfs::new(&gr, n(0));
    while dfs.next(&gr).is_some() {}

    let map = dfs.discovered;
    gr.add_edge(n(0), n(1), ());
    let mut po = Vec::new();
    let mut dfs = DfsPostOrder::new(&gr, n(0));
    let f = NodeFiltered(&gr, map);
    while let Some(n) = dfs.next(&f) {
        po.push(n);
    }
    assert!(!po.contains(&n(1)));
}

#[test]
fn filter_elements() {
    use petgraph::data::Element::{Edge, Node};
    use petgraph::data::ElementIterator;
    use petgraph::data::FromElements;
    let elements = vec![
        Node { weight: "A" },
        Node { weight: "B" },
        Node { weight: "C" },
        Node { weight: "D" },
        Node { weight: "E" },
        Node { weight: "F" },
        Edge {
            source: 0,
            target: 1,
            weight: 7,
        },
        Edge {
            source: 2,
            target: 0,
            weight: 9,
        },
        Edge {
            source: 0,
            target: 3,
            weight: 14,
        },
        Edge {
            source: 1,
            target: 2,
            weight: 10,
        },
        Edge {
            source: 3,
            target: 2,
            weight: 2,
        },
        Edge {
            source: 3,
            target: 4,
            weight: 9,
        },
        Edge {
            source: 1,
            target: 5,
            weight: 15,
        },
        Edge {
            source: 2,
            target: 5,
            weight: 11,
        },
        Edge {
            source: 4,
            target: 5,
            weight: 6,
        },
    ];
    let mut g = DiGraph::<_, _>::from_elements(elements.iter().cloned());
    println!("{:#?}", g);
    assert!(g.contains_edge(n(1), n(5)));
    let g2 =
        DiGraph::<_, _>::from_elements(elements.iter().cloned().filter_elements(|elt| match elt {
            Node { ref weight } if **weight == "B" => false,
            _ => true,
        }));
    println!("{:#?}", g2);
    g.remove_node(n(1));
    assert!(is_isomorphic_matching(
        &g,
        &g2,
        PartialEq::eq,
        PartialEq::eq
    ));
}

#[test]
fn test_edge_filtered() {
    use petgraph::algo::connected_components;
    use petgraph::visit::EdgeFiltered;
    use petgraph::visit::IntoEdgeReferences;

    let gr = UnGraph::<(), _>::from_edges(&[
        // cycle
        (0, 1, 7),
        (1, 2, 9),
        (2, 1, 14),
        // cycle
        (3, 4, 10),
        (4, 5, 2),
        (5, 3, 9),
        // cross edges
        (0, 3, -1),
        (1, 4, -2),
        (2, 5, -3),
    ]);
    assert_eq!(connected_components(&gr), 1);
    let positive_edges = EdgeFiltered::from_fn(&gr, |edge| *edge.weight() >= 0);
    assert_eq!(positive_edges.edge_references().count(), 6);
    assert!(positive_edges
        .edge_references()
        .all(|edge| *edge.weight() >= 0));
    assert_eq!(connected_components(&positive_edges), 2);

    let mut dfs = DfsPostOrder::new(&positive_edges, n(0));
    while dfs.next(&positive_edges).is_some() {}

    let n = n::<u32>;
    for node in &[n(0), n(1), n(2)] {
        assert!(dfs.discovered.is_visited(node));
    }
    for node in &[n(3), n(4), n(5)] {
        assert!(!dfs.discovered.is_visited(node));
    }
}

#[test]
fn test_dominators_simple_fast() {
    // Construct the following graph:
    //
    //                                  .-----.
    //                                  |     <--------------------------------.
    //          .--------+--------------|  r  |--------------.                 |
    //          |        |              |     |              |                 |
    //          |        |              '-----'              |                 |
    //          |     .--V--.                             .--V--.              |
    //          |     |     |                             |     |              |
    //          |     |  b  |                             |  c  |--------.     |
    //          |     |     |                             |     |        |     |
    //          |     '-----'                             '-----'        |     |
    //       .--V--.     |                                   |        .--V--.  |
    //       |     |     |                                   |        |     |  |
    //       |  a  <-----+                                   |   .----|  g  |  |
    //       |     |     |                              .----'   |    |     |  |
    //       '-----'     |                              |        |    '-----'  |
    //          |        |                              |        |       |     |
    //       .--V--.     |    .-----.                .--V--.     |       |     |
    //       |     |     |    |     |                |     |     |       |     |
    //       |  d  <-----+---->  e  <----.           |  f  |     |       |     |
    //       |     |          |     |    |           |     |     |       |     |
    //       '-----'          '-----'    |           '-----'     |       |     |
    //          |     .-----.    |       |              |        |    .--V--.  |
    //          |     |     |    |       |              |      .-'    |     |  |
    //          '----->  l  |    |       |              |      |      |  j  |  |
    //                |     |    '--.    |              |      |      |     |  |
    //                '-----'       |    |              |      |      '-----'  |
    //                   |       .--V--. |              |   .--V--.      |     |
    //                   |       |     | |              |   |     |      |     |
    //                   '------->  h  |-'              '--->  i  <------'     |
    //                           |     |          .--------->     |            |
    //                           '-----'          |         '-----'            |
    //                              |          .-----.         |               |
    //                              |          |     |         |               |
    //                              '---------->  k  <---------'               |
    //                                         |     |                         |
    //                                         '-----'                         |
    //                                            |                            |
    //                                            '----------------------------'
    //
    // This graph has the following dominator tree:
    //
    //     r
    //     |-- a
    //     |-- b
    //     |-- c
    //     |   |-- f
    //     |   `-- g
    //     |       `-- j
    //     |-- d
    //     |   `-- l
    //     |-- e
    //     |-- i
    //     |-- k
    //     `-- h
    //
    // This graph and dominator tree are taken from figures 1 and 2 of "A Fast
    // Algorithm for Finding Dominators in a Flowgraph" by Lengauer et al:
    // http://www.cs.princeton.edu/courses/archive/spr03/cs423/download/dominators.pdf.

    let mut graph = DiGraph::<_, _>::new();

    let r = graph.add_node("r");
    let a = graph.add_node("a");
    let b = graph.add_node("b");
    let c = graph.add_node("c");
    let d = graph.add_node("d");
    let e = graph.add_node("e");
    let f = graph.add_node("f");
    let g = graph.add_node("g");
    let h = graph.add_node("h");
    let i = graph.add_node("i");
    let j = graph.add_node("j");
    let k = graph.add_node("k");
    let l = graph.add_node("l");

    graph.add_edge(r, a, ());
    graph.add_edge(r, b, ());
    graph.add_edge(r, c, ());
    graph.add_edge(a, d, ());
    graph.add_edge(b, a, ());
    graph.add_edge(b, d, ());
    graph.add_edge(b, e, ());
    graph.add_edge(c, f, ());
    graph.add_edge(c, g, ());
    graph.add_edge(d, l, ());
    graph.add_edge(e, h, ());
    graph.add_edge(f, i, ());
    graph.add_edge(g, i, ());
    graph.add_edge(g, j, ());
    graph.add_edge(h, e, ());
    graph.add_edge(h, k, ());
    graph.add_edge(i, k, ());
    graph.add_edge(j, i, ());
    graph.add_edge(k, r, ());
    graph.add_edge(k, i, ());
    graph.add_edge(l, h, ());

    let doms = dominators::simple_fast(&graph, r);

    assert_eq!(doms.root(), r);
    assert_eq!(
        doms.immediate_dominator(r),
        None,
        "The root node has no idom"
    );

    assert_eq!(
        doms.immediate_dominator(a),
        Some(r),
        "r is the immediate dominator of a"
    );
    assert_eq!(
        doms.immediate_dominator(b),
        Some(r),
        "r is the immediate dominator of b"
    );
    assert_eq!(
        doms.immediate_dominator(c),
        Some(r),
        "r is the immediate dominator of c"
    );
    assert_eq!(
        doms.immediate_dominator(f),
        Some(c),
        "c is the immediate dominator of f"
    );
    assert_eq!(
        doms.immediate_dominator(g),
        Some(c),
        "c is the immediate dominator of g"
    );
    assert_eq!(
        doms.immediate_dominator(j),
        Some(g),
        "g is the immediate dominator of j"
    );
    assert_eq!(
        doms.immediate_dominator(d),
        Some(r),
        "r is the immediate dominator of d"
    );
    assert_eq!(
        doms.immediate_dominator(l),
        Some(d),
        "d is the immediate dominator of l"
    );
    assert_eq!(
        doms.immediate_dominator(e),
        Some(r),
        "r is the immediate dominator of e"
    );
    assert_eq!(
        doms.immediate_dominator(i),
        Some(r),
        "r is the immediate dominator of i"
    );
    assert_eq!(
        doms.immediate_dominator(k),
        Some(r),
        "r is the immediate dominator of k"
    );
    assert_eq!(
        doms.immediate_dominator(h),
        Some(r),
        "r is the immediate dominator of h"
    );

    let mut graph = graph.clone();
    let z = graph.add_node("z");
    let doms = dominators::simple_fast(&graph, r);
    assert_eq!(
        doms.immediate_dominator(z),
        None,
        "nodes that aren't reachable from the root do not have an idom"
    );
}
