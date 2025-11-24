#![cfg(feature = "graphmap")]
extern crate petgraph;

use std::collections::HashSet;
use std::fmt;

use petgraph::prelude::*;
use petgraph::visit::Walker;

use petgraph::algo::dijkstra;

use petgraph::dot::{Config, Dot};

#[test]
fn simple() {
    //let root = TypedArena::<Node<_>>::new();
    let mut gr = UnGraphMap::new();
    //let node = |&: name: &'static str| Ptr(root.alloc(Node(name.to_string())));
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let f = gr.add_node("F");
    gr.add_edge(a, b, 7);
    gr.add_edge(a, c, 9);
    gr.add_edge(a, d, 14);
    gr.add_edge(b, c, 10);
    gr.add_edge(c, d, 2);
    gr.add_edge(d, e, 9);
    gr.add_edge(b, f, 15);
    gr.add_edge(c, f, 11);

    assert!(gr.add_edge(e, f, 5).is_none());

    // duplicate edges
    assert_eq!(gr.add_edge(f, b, 16), Some(15));
    assert_eq!(gr.add_edge(f, e, 6), Some(5));
    println!("{:?}", gr);
    println!("{}", Dot::with_config(&gr, &[]));

    assert_eq!(gr.node_count(), 6);
    assert_eq!(gr.edge_count(), 9);

    // check updated edge weight
    assert_eq!(gr.edge_weight(e, f), Some(&6));
    let scores = dijkstra(&gr, a, None, |e| *e.weight());
    let mut scores: Vec<_> = scores.into_iter().collect();
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
}

#[test]
fn edges_directed() {
    let mut gr = DiGraphMap::new();
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let f = gr.add_node("F");
    gr.add_edge(a, b, 7);
    gr.add_edge(a, c, 9);
    gr.add_edge(a, d, 14);
    gr.add_edge(b, c, 10);
    gr.add_edge(c, d, 2);
    gr.add_edge(d, e, 9);
    gr.add_edge(b, f, 15);
    gr.add_edge(c, f, 11);

    let mut edges_out = gr.edges_directed(c, Direction::Outgoing);
    assert_eq!(edges_out.next(), Some((c, d, &2)));
    assert_eq!(edges_out.next(), Some((c, f, &11)));
    assert_eq!(edges_out.next(), None);

    let mut edges_in = gr.edges_directed(c, Direction::Incoming);
    assert_eq!(edges_in.next(), Some((a, c, &9)));
    assert_eq!(edges_in.next(), Some((b, c, &10)));
    assert_eq!(edges_in.next(), None);
}

#[test]
fn remov() {
    let mut g = UnGraphMap::new();
    g.add_node(1);
    g.add_node(2);
    g.add_edge(1, 2, -1);

    assert_eq!(g.edge_weight(1, 2), Some(&-1));
    assert_eq!(g.edge_weight(2, 1), Some(&-1));
    assert_eq!(g.neighbors(1).count(), 1);

    let noexist = g.remove_edge(2, 3);
    assert_eq!(noexist, None);

    let exist = g.remove_edge(2, 1);
    assert_eq!(exist, Some(-1));
    assert_eq!(g.edge_count(), 0);
    assert_eq!(g.edge_weight(1, 2), None);
    assert_eq!(g.edge_weight(2, 1), None);
    assert_eq!(g.neighbors(1).count(), 0);
}

#[test]
fn remove_node() {
    // From #431
    let mut graph = petgraph::graphmap::DiGraphMap::<u32, ()>::new();
    graph.add_edge(1, 2, ());
    graph.remove_node(2);

    let neighbors: Vec<u32> = graph.neighbors(1).collect();
    assert_eq!(neighbors, []);

    let edges: Vec<(u32, u32, _)> = graph.all_edges().collect();
    assert_eq!(edges, []);
}

#[test]
fn remove_directed() {
    let mut g = GraphMap::<_, _, Directed>::with_capacity(0, 0);
    g.add_edge(1, 2, -1);
    println!("{:?}", g);

    assert_eq!(g.edge_weight(1, 2), Some(&-1));
    assert_eq!(g.edge_weight(2, 1), None);
    assert_eq!(g.neighbors(1).count(), 1);

    let noexist = g.remove_edge(2, 3);
    assert_eq!(noexist, None);

    let exist = g.remove_edge(2, 1);
    assert_eq!(exist, None);
    let exist = g.remove_edge(1, 2);
    assert_eq!(exist, Some(-1));
    println!("{:?}", g);
    assert_eq!(g.edge_count(), 0);
    assert_eq!(g.edge_weight(1, 2), None);
    assert_eq!(g.edge_weight(2, 1), None);
    assert_eq!(g.neighbors(1).count(), 0);
}

#[test]
fn dfs() {
    let mut gr = UnGraphMap::default();
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    let k = gr.add_node("K");
    // Z is disconnected.
    let z = gr.add_node("Z");
    gr.add_edge(h, i, 1.);
    gr.add_edge(h, j, 3.);
    gr.add_edge(i, j, 1.);
    gr.add_edge(i, k, 2.);

    println!("{:?}", gr);

    {
        let mut cnt = 0;
        let mut dfs = Dfs::new(&gr, h);
        while dfs.next(&gr).is_some() {
            cnt += 1;
        }
        assert_eq!(cnt, 4);
    }
    {
        let mut cnt = 0;
        let mut dfs = Dfs::new(&gr, z);
        while dfs.next(&gr).is_some() {
            cnt += 1;
        }
        assert_eq!(cnt, 1);
    }

    assert_eq!(Dfs::new(&gr, h).iter(&gr).count(), 4);
    assert_eq!(Dfs::new(&gr, i).iter(&gr).count(), 4);
    assert_eq!(Dfs::new(&gr, z).iter(&gr).count(), 1);
}

#[test]
fn edge_iterator() {
    let mut gr = UnGraphMap::new();
    let h = gr.add_node("H");
    let i = gr.add_node("I");
    let j = gr.add_node("J");
    let k = gr.add_node("K");
    gr.add_edge(h, i, 1);
    gr.add_edge(h, j, 2);
    gr.add_edge(i, j, 3);
    gr.add_edge(i, k, 4);

    let real_edges: HashSet<_> = gr.all_edges().map(|(a, b, &w)| (a, b, w)).collect();
    let expected_edges: HashSet<_> =
        vec![("H", "I", 1), ("H", "J", 2), ("I", "J", 3), ("I", "K", 4)]
            .into_iter()
            .collect();

    assert_eq!(real_edges, expected_edges);
}

#[test]
fn from_edges() {
    let gr =
        GraphMap::<_, _, Undirected>::from_edges(&[("a", "b", 1), ("a", "c", 2), ("c", "d", 3)]);
    assert_eq!(gr.node_count(), 4);
    assert_eq!(gr.edge_count(), 3);
    assert_eq!(gr[("a", "c")], 2);

    let gr = GraphMap::<_, (), Undirected>::from_edges(&[
        (0, 1),
        (0, 2),
        (0, 3),
        (1, 2),
        (1, 3),
        (2, 3),
    ]);
    assert_eq!(gr.node_count(), 4);
    assert_eq!(gr.edge_count(), 6);
    assert_eq!(gr.neighbors(0).count(), 3);
    assert_eq!(gr.neighbors(1).count(), 3);
    assert_eq!(gr.neighbors(2).count(), 3);
    assert_eq!(gr.neighbors(3).count(), 3);

    println!("{:?}", Dot::with_config(&gr, &[Config::EdgeNoLabel]));
}

#[test]
fn graphmap_directed() {
    //let root = TypedArena::<Node<_>>::new();
    let mut gr = DiGraphMap::<_, ()>::with_capacity(0, 0);
    //let node = |&: name: &'static str| Ptr(root.alloc(Node(name.to_string())));
    let a = gr.add_node("A");
    let b = gr.add_node("B");
    let c = gr.add_node("C");
    let d = gr.add_node("D");
    let e = gr.add_node("E");
    let edges = [(a, b), (a, c), (a, d), (b, c), (c, d), (d, e), (b, b)];
    gr.extend(&edges);

    // Add reverse edges -- ok!
    assert!(gr.add_edge(e, d, ()).is_none());
    // duplicate edge - no
    assert!(gr.add_edge(a, b, ()).is_some());

    // duplicate self loop - no
    assert!(gr.add_edge(b, b, ()).is_some());
    println!("{:#?}", gr);
}

fn assert_sccs_eq<N>(mut res: Vec<Vec<N>>, mut answer: Vec<Vec<N>>)
where
    N: Ord + fmt::Debug,
{
    // normalize the result and compare with the answer.
    for scc in &mut res {
        scc.sort();
    }
    res.sort();
    for scc in &mut answer {
        scc.sort();
    }
    answer.sort();
    assert_eq!(res, answer);
}

#[test]
fn scc() {
    let gr: GraphMap<_, u32, Directed> = GraphMap::from_edges(&[
        (6, 0, 0),
        (0, 3, 1),
        (3, 6, 2),
        (8, 6, 3),
        (8, 2, 4),
        (2, 5, 5),
        (5, 8, 6),
        (7, 5, 7),
        (1, 7, 8),
        (7, 4, 9),
        (4, 1, 10),
    ]);

    assert_sccs_eq(
        petgraph::algo::kosaraju_scc(&gr),
        vec![vec![0, 3, 6], vec![1, 4, 7], vec![2, 5, 8]],
    );
}

#[test]
fn test_into_graph() {
    let gr: GraphMap<_, u32, Directed> = GraphMap::from_edges(&[
        (6, 0, 0),
        (0, 3, 1),
        (3, 6, 2),
        (8, 6, 3),
        (8, 2, 4),
        (2, 5, 5),
        (5, 8, 6),
        (7, 5, 7),
        (1, 7, 8),
        (7, 4, 9),
        (4, 1, 10),
    ]);

    let graph: Graph<_, _, _> = gr.clone().into_graph();
    println!("{}", Dot::new(&gr));
    println!("{}", Dot::new(&graph));

    // node weigths in `graph` are node identifiers in `gr`.
    for edge in graph.edge_references() {
        let a = edge.source();
        let b = edge.target();
        let aw = graph[a];
        let bw = graph[b];
        assert_eq!(&gr[(aw, bw)], edge.weight());
    }
}

#[test]
fn test_from_graph() {
    let mut gr: Graph<u32, u32, Directed> = Graph::new();
    let node_a = gr.add_node(12);
    let node_b = gr.add_node(13);
    let node_c = gr.add_node(14);
    gr.add_edge(node_a, node_b, 1000);
    gr.add_edge(node_b, node_c, 999);
    gr.add_edge(node_c, node_a, 1111);
    gr.add_node(42);
    let gr = gr;

    let graph: GraphMap<u32, u32, Directed> = GraphMap::from_graph(gr.clone());
    println!("{}", Dot::new(&gr));
    println!("{}", Dot::new(&graph));

    assert!(petgraph::algo::is_isomorphic(&gr, &graph));
    assert_eq!(graph[(12, 13)], 1000);
}

#[test]
fn test_all_edges_mut() {
    // graph with edge weights equal to in+out
    let mut graph: GraphMap<_, u32, Directed> =
        GraphMap::from_edges(&[(0, 1, 1), (1, 2, 3), (2, 0, 2)]);

    // change it so edge weight is equal to 2 * (in+out)
    for (start, end, weight) in graph.all_edges_mut() {
        *weight = (start + end) * 2;
    }

    // test it
    for (start, end, weight) in graph.all_edges() {
        assert_eq!((start + end) * 2, *weight);
    }
}

#[test]
fn neighbors_incoming_includes_self_loops() {
    let mut graph = DiGraphMap::new();

    graph.add_node(());
    graph.add_edge((), (), ());

    let mut neighbors = graph.neighbors_directed((), Incoming);
    assert_eq!(neighbors.next(), Some(()));
    assert_eq!(neighbors.next(), None);
}

#[test]
fn undirected_neighbors_includes_self_loops() {
    let mut graph = UnGraphMap::new();

    graph.add_node(());
    graph.add_edge((), (), ());

    let mut neighbors = graph.neighbors(());
    assert_eq!(neighbors.next(), Some(()));
    assert_eq!(neighbors.next(), None);
}

#[test]
fn self_loops_can_be_removed() {
    let mut graph = DiGraphMap::new();

    graph.add_node(());
    graph.add_edge((), (), ());

    graph.remove_edge((), ());

    assert_eq!(graph.neighbors_directed((), Outgoing).next(), None);
    assert_eq!(graph.neighbors_directed((), Incoming).next(), None);
}

#[test]
#[cfg(feature = "rayon")]
fn test_parallel_iterator() {
    use rayon::prelude::*;
    let mut gr: DiGraphMap<u32, u32> = DiGraphMap::new();

    for i in 0..1000 {
        gr.add_node(i);
    }

    let serial_sum: u32 = gr.nodes().sum();
    let parallel_sum: u32 = gr.par_nodes().sum();
    assert_eq!(serial_sum, parallel_sum);

    gr.par_nodes()
        .enumerate()
        .for_each(|(i, n)| assert_eq!(i as u32, n));

    for i in 0..1000 {
        gr.add_edge(i / 2, i, i + i / 2);
    }

    let serial_sum: u32 = gr.all_edges().map(|(.., &e)| e).sum();
    let parallel_sum: u32 = gr.par_all_edges().map(|(.., &e)| e).sum();
    assert_eq!(serial_sum, parallel_sum);

    gr.par_all_edges_mut().for_each(|(n1, n2, e)| *e -= n1 + n2);
    gr.all_edges().for_each(|(.., &e)| assert_eq!(e, 0));
}

#[test]
fn test_alternative_hasher() {
    let mut gr: GraphMap<&str, u32, Directed, fxhash::FxBuildHasher> = GraphMap::new();
    gr.add_node("abc");
    gr.add_node("def");
    gr.add_node("ghi");

    gr.add_edge("abc", "def", 1);

    assert!(gr.contains_edge("abc", "def"));
    assert!(!gr.contains_edge("abc", "ghi"));
}
