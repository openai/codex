#![feature(test)]

extern crate petgraph;
extern crate test;

use test::Bencher;

use petgraph::algo;
use petgraph::matrix_graph::{node_index, MatrixGraph};
use petgraph::{Directed, EdgeType, Incoming, Outgoing};

#[bench]
fn add_100_nodes(b: &mut test::Bencher) {
    b.iter(|| {
        let mut g = MatrixGraph::<(), ()>::with_capacity(100);

        for _ in 0..100 {
            let _ = g.add_node(());
        }
    });
}

#[bench]
fn add_100_edges_to_self(b: &mut test::Bencher) {
    let mut g = MatrixGraph::<(), ()>::with_capacity(100);
    let nodes: Vec<_> = (0..100).map(|_| g.add_node(())).collect();
    let g = g;

    b.iter(|| {
        let mut g = g.clone();

        for &node in nodes.iter() {
            g.add_edge(node, node, ());
        }
    });
}

#[bench]
fn add_5_edges_for_each_of_100_nodes(b: &mut test::Bencher) {
    let mut g = MatrixGraph::<(), ()>::with_capacity(100);
    let nodes: Vec<_> = (0..100).map(|_| g.add_node(())).collect();
    let g = g;

    let edges_to_add: Vec<_> = nodes
        .iter()
        .enumerate()
        .flat_map(|(i, &node)| {
            let edges: Vec<_> = (0..5)
                .map(|j| (i + j + 1) % nodes.len())
                .map(|j| (node, nodes[j]))
                .collect();

            edges
        })
        .collect();

    b.iter(|| {
        let mut g = g.clone();

        for &(source, target) in edges_to_add.iter() {
            g.add_edge(source, target, ());
        }
    });
}

#[bench]
fn add_edges_from_root(bench: &mut test::Bencher) {
    bench.iter(|| {
        let mut gr = MatrixGraph::new();
        let a = gr.add_node(());

        for _ in 0..100 {
            let b = gr.add_node(());
            gr.add_edge(a, b, ());
        }
    });
}

#[bench]
fn add_adjacent_edges(bench: &mut test::Bencher) {
    bench.iter(|| {
        let mut gr = MatrixGraph::new();
        let mut prev = None;
        for _ in 0..100 {
            let b = gr.add_node(());

            if let Some(a) = prev {
                gr.add_edge(a, b, ());
            }

            prev = Some(b);
        }
    });
}

/// An almost full set
const FULL: &str = "
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 1 1 1 1 1 1
 1 1 1 1 0 1 1 1 0 1
 1 1 1 1 1 1 1 1 1 1
";

const BIGGER: &str = "
 0 0 0 0 0 0 0 0 1 0 1 0 0 0 0 0 0 0 1 1 0 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 1 0 0 0 1 1 0 0 0 0 0 0 0 1 0 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 0 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 1 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0
 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 0 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0
 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1
 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 1 1 0 1 1 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1
 0 0 0 0 1 0 0 0 0 0 0 0 1 0 0 0 1 0 1 1 0 1 1 1 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1
 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 1 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0
 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 1 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0
 1 0 1 0 0 1 0 0 0 1 0 0 0 0 0 0 0 1 1 0 1 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0
 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 1 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0
 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 0 0 0 1 0 0 0
 0 1 0 0 0 0 0 0 1 0 1 1 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 0 1 0 1 1 0 0 0 0 0 1 0 0
 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 0 0 0 1 0
 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 1 0 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1
 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 1 1 1 0 1 0 0
 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1 1 1 0 0 0
 0 1 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1 0 0 0 0 0 0 1 0 0 0 0 0 1 1 0 1 0 0 0 1
 0 1 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 0 0 1 0
 0 1 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1
 0 1 0 0 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1 0 0 0 0 0 0 0 0 0 1 0 0 1 0 0 0 1 0 1 1
 0 1 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 1 1 0 1
 0 1 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 1 0 1 1 1 0
";

/// Parse a text adjacency matrix format into a directed graph
fn parse_matrix<Ty: EdgeType>(s: &str) -> MatrixGraph<(), (), Ty> {
    let mut gr = MatrixGraph::default();
    let s = s.trim();
    let lines = s.lines().filter(|l| !l.is_empty());
    for (row, line) in lines.enumerate() {
        for (col, word) in line.split(' ').filter(|s| !s.is_empty()).enumerate() {
            let has_edge = word.parse::<i32>().unwrap();
            assert!(has_edge == 0 || has_edge == 1);
            if has_edge == 0 {
                continue;
            }
            while col >= gr.node_count() || row >= gr.node_count() {
                gr.add_node(());
            }
            gr.add_edge(node_index(row), node_index(col), ());
        }
    }
    gr
}

#[bench]
fn full_edges_out(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(FULL);
    bench.iter(|| a.edges_directed(node_index(1), Outgoing).count())
}

#[bench]
fn full_edges_in(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(FULL);
    bench.iter(|| a.edges_directed(node_index(1), Incoming).count())
}

#[bench]
fn full_neighbors_out(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(FULL);
    bench.iter(|| a.neighbors_directed(node_index(1), Outgoing).count())
}

#[bench]
fn full_neighbors_in(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(FULL);

    bench.iter(|| a.neighbors_directed(node_index(1), Incoming).count())
}

#[bench]
fn full_kosaraju_sccs(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(FULL);
    bench.iter(|| algo::kosaraju_scc(&a));
}

#[bench]
fn full_tarjan_sccs(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(FULL);
    bench.iter(|| algo::tarjan_scc(&a));
}

#[bench]
fn bigger_edges_out(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(BIGGER);
    bench.iter(|| a.edges_directed(node_index(1), Outgoing).count())
}

#[bench]
fn bigger_edges_in(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(BIGGER);
    bench.iter(|| a.edges_directed(node_index(1), Incoming).count())
}

#[bench]
fn bigger_neighbors_out(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(BIGGER);
    bench.iter(|| a.neighbors_directed(node_index(1), Outgoing).count())
}

#[bench]
fn bigger_neighbors_in(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(BIGGER);

    bench.iter(|| a.neighbors_directed(node_index(1), Incoming).count())
}

#[bench]
fn bigger_kosaraju_sccs(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(BIGGER);
    bench.iter(|| algo::kosaraju_scc(&a));
}

#[bench]
fn bigger_tarjan_sccs(bench: &mut Bencher) {
    let a = parse_matrix::<Directed>(BIGGER);
    bench.iter(|| algo::tarjan_scc(&a));
}
