#![feature(test)]

extern crate petgraph;
extern crate test;

use test::Bencher;

#[allow(dead_code)]
mod common;
use common::*;

use petgraph::algo::{greedy_matching, maximum_matching};
use petgraph::graph::UnGraph;

fn huge() -> UnGraph<(), ()> {
    static NODE_COUNT: u32 = 1_000;

    let mut edges = Vec::new();

    for i in 0..NODE_COUNT {
        for j in i..NODE_COUNT {
            if i % 3 == 0 && j % 2 == 0 {
                edges.push((i, j));
            }
        }
    }

    // 999 nodes, 83500 edges
    UnGraph::from_edges(&edges)
}

#[bench]
fn greedy_matching_bipartite(bench: &mut Bencher) {
    let g = ungraph().bipartite();
    bench.iter(|| greedy_matching(&g));
}

#[bench]
fn greedy_matching_full(bench: &mut Bencher) {
    let g = ungraph().full_a();
    bench.iter(|| greedy_matching(&g));
}

#[bench]
fn greedy_matching_bigger(bench: &mut Bencher) {
    let g = ungraph().bigger();
    bench.iter(|| greedy_matching(&g));
}

#[bench]
fn greedy_matching_huge(bench: &mut Bencher) {
    let g = huge();
    bench.iter(|| greedy_matching(&g));
}

#[bench]
fn maximum_matching_bipartite(bench: &mut Bencher) {
    let g = ungraph().bipartite();
    bench.iter(|| maximum_matching(&g));
}

#[bench]
fn maximum_matching_full(bench: &mut Bencher) {
    let g = ungraph().full_a();
    bench.iter(|| maximum_matching(&g));
}

#[bench]
fn maximum_matching_bigger(bench: &mut Bencher) {
    let g = ungraph().bigger();
    bench.iter(|| maximum_matching(&g));
}

#[bench]
fn maximum_matching_huge(bench: &mut Bencher) {
    let g = huge();
    bench.iter(|| maximum_matching(&g));
}
