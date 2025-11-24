#![feature(test)]

extern crate petgraph;
extern crate test;

use petgraph::graph::Graph;

#[bench]
fn bench_inser(b: &mut test::Bencher) {
    let mut og = Graph::new();
    let fst = og.add_node(0i32);
    for x in 1..125 {
        let n = og.add_node(x);
        og.add_edge(fst, n, ());
    }
    b.iter(|| og.add_node(1))
}

#[bench]
fn bench_add_edge(b: &mut test::Bencher) {
    let mut og = Graph::new();
    for _ in 0..100 {
        og.add_node(());
    }

    b.iter(|| {
        for (a, b) in og.node_indices().zip(og.node_indices().skip(1)) {
            og.add_edge(a, b, ());
        }
        og.clear_edges();
    })
}

#[bench]
fn bench_remove(b: &mut test::Bencher) {
    // removal is very slow in a big graph.
    // and this one doesn't even have many nodes.
    let mut og = Graph::new();
    let fst = og.add_node(0i32);
    let mut prev = fst;
    for x in 1..1250 {
        let n = og.add_node(x);
        og.add_edge(prev, n, ());
        prev = n;
    }
    //println!("{}", og);
    b.iter(|| {
        for _ in 0..100 {
            og.remove_node(fst);
        }
    })
}
