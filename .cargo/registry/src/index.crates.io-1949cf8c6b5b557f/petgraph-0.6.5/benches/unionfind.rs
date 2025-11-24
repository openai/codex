#![feature(test)]

extern crate petgraph;
extern crate test;

use test::Bencher;

#[allow(dead_code)]
mod common;
use common::*;

use petgraph::algo::{connected_components, is_cyclic_undirected};

#[bench]
fn connected_components_praust_undir_bench(bench: &mut Bencher) {
    let a = ungraph().praust_a();
    let b = ungraph().praust_b();

    bench.iter(|| (connected_components(&a), connected_components(&b)));
}

#[bench]
fn connected_components_praust_dir_bench(bench: &mut Bencher) {
    let a = digraph().praust_a();
    let b = digraph().praust_b();

    bench.iter(|| (connected_components(&a), connected_components(&b)));
}

#[bench]
fn connected_components_full_undir_bench(bench: &mut Bencher) {
    let a = ungraph().full_a();
    let b = ungraph().full_b();

    bench.iter(|| (connected_components(&a), connected_components(&b)));
}

#[bench]
fn connected_components_full_dir_bench(bench: &mut Bencher) {
    let a = digraph().full_a();
    let b = digraph().full_b();

    bench.iter(|| (connected_components(&a), connected_components(&b)));
}

#[bench]
fn connected_components_petersen_undir_bench(bench: &mut Bencher) {
    let a = ungraph().petersen_a();
    let b = ungraph().petersen_b();

    bench.iter(|| (connected_components(&a), connected_components(&b)));
}

#[bench]
fn connected_components_petersen_dir_bench(bench: &mut Bencher) {
    let a = digraph().petersen_a();
    let b = digraph().petersen_b();

    bench.iter(|| (connected_components(&a), connected_components(&b)));
}

#[bench]
fn is_cyclic_undirected_praust_undir_bench(bench: &mut Bencher) {
    let a = ungraph().praust_a();
    let b = ungraph().praust_b();

    bench.iter(|| (is_cyclic_undirected(&a), is_cyclic_undirected(&b)));
}

#[bench]
fn is_cyclic_undirected_praust_dir_bench(bench: &mut Bencher) {
    let a = digraph().praust_a();
    let b = digraph().praust_b();

    bench.iter(|| (is_cyclic_undirected(&a), is_cyclic_undirected(&b)));
}

#[bench]
fn is_cyclic_undirected_full_undir_bench(bench: &mut Bencher) {
    let a = ungraph().full_a();
    let b = ungraph().full_b();

    bench.iter(|| (is_cyclic_undirected(&a), is_cyclic_undirected(&b)));
}

#[bench]
fn is_cyclic_undirected_full_dir_bench(bench: &mut Bencher) {
    let a = digraph().full_a();
    let b = digraph().full_b();

    bench.iter(|| (is_cyclic_undirected(&a), is_cyclic_undirected(&b)));
}

#[bench]
fn is_cyclic_undirected_petersen_undir_bench(bench: &mut Bencher) {
    let a = ungraph().petersen_a();
    let b = ungraph().petersen_b();

    bench.iter(|| (is_cyclic_undirected(&a), is_cyclic_undirected(&b)));
}

#[bench]
fn is_cyclic_undirected_petersen_dir_bench(bench: &mut Bencher) {
    let a = digraph().petersen_a();
    let b = digraph().petersen_b();

    bench.iter(|| (is_cyclic_undirected(&a), is_cyclic_undirected(&b)));
}
