#![feature(test)]

extern crate petgraph;
extern crate test;

use petgraph::prelude::*;
use test::Bencher;

#[allow(dead_code)]
mod common;
use common::*;

use petgraph::stable_graph::node_index;

#[bench]
fn full_edges_default(bench: &mut Bencher) {
    let a = stable_digraph().full_a();
    bench.iter(|| a.edges(node_index(1)).count())
}

#[bench]
fn full_edges_out(bench: &mut Bencher) {
    let a = stable_digraph().full_a();
    bench.iter(|| a.edges_directed(node_index(1), Outgoing).count())
}

#[bench]
fn full_edges_in(bench: &mut Bencher) {
    let a = stable_digraph().full_a();
    bench.iter(|| a.edges_directed(node_index(1), Incoming).count())
}

#[bench]
fn neighbors_default(bench: &mut Bencher) {
    let a = stable_digraph().full_a();
    bench.iter(|| a.neighbors(node_index(1)).count())
}

#[bench]
fn neighbors_out(bench: &mut Bencher) {
    let a = stable_digraph().full_a();
    bench.iter(|| a.neighbors_directed(node_index(1), Outgoing).count())
}

#[bench]
fn neighbors_in(bench: &mut Bencher) {
    let a = stable_digraph().full_a();
    bench.iter(|| a.neighbors_directed(node_index(1), Incoming).count())
}

#[bench]
fn sccs_kosaraju_stable_graph(bench: &mut Bencher) {
    let a = stable_digraph().bigger();
    bench.iter(|| petgraph::algo::kosaraju_scc(&a));
}

#[bench]
fn sccs_kosaraju_graph(bench: &mut Bencher) {
    let a = digraph().bigger();
    bench.iter(|| petgraph::algo::kosaraju_scc(&a));
}

#[bench]
fn sccs_tarjan_stable_graph(bench: &mut Bencher) {
    let a = stable_digraph().bigger();
    bench.iter(|| petgraph::algo::tarjan_scc(&a));
}

#[bench]
fn sccs_tarjan_graph(bench: &mut Bencher) {
    let a = digraph().bigger();
    bench.iter(|| petgraph::algo::tarjan_scc(&a));
}

#[bench]
fn stable_graph_map(bench: &mut Bencher) {
    let a = stable_digraph().bigger();
    bench.iter(|| a.map(|i, _| i, |i, _| i));
}

#[bench]
fn graph_map(bench: &mut Bencher) {
    let a = digraph().bigger();
    bench.iter(|| a.map(|i, _| i, |i, _| i));
}

#[bench]
fn stable_graph_retain_nodes(bench: &mut Bencher) {
    let mut a = stable_digraph().bigger();
    bench.iter(|| a.retain_nodes(|_gr, i| (i.index() + 1) % 3700 != 0));
}

#[bench]
fn stable_graph_retain_edges(bench: &mut Bencher) {
    let mut a = stable_digraph().bigger();
    bench.iter(|| a.retain_edges(|_gr, i| (i.index() + 1) % 3700 != 0));
}
