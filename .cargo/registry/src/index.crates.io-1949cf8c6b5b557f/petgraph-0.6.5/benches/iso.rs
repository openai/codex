#![feature(test)]

extern crate petgraph;
extern crate test;

use test::Bencher;

#[allow(dead_code)]
mod common;
use common::*;

use petgraph::algo::is_isomorphic;

#[bench]
fn petersen_iso_bench(bench: &mut Bencher) {
    let a = digraph().petersen_a();
    let b = digraph().petersen_b();

    bench.iter(|| is_isomorphic(&a, &b));
    assert!(is_isomorphic(&a, &b));
}

#[bench]
fn petersen_undir_iso_bench(bench: &mut Bencher) {
    let a = ungraph().petersen_a();
    let b = ungraph().petersen_b();

    bench.iter(|| is_isomorphic(&a, &b));
    assert!(is_isomorphic(&a, &b));
}

#[bench]
fn full_iso_bench(bench: &mut Bencher) {
    let a = ungraph().full_a();
    let b = ungraph().full_b();

    bench.iter(|| is_isomorphic(&a, &b));
    assert!(is_isomorphic(&a, &b));
}

#[bench]
fn praust_dir_no_iso_bench(bench: &mut Bencher) {
    let a = digraph().praust_a();
    let b = digraph().praust_b();

    bench.iter(|| is_isomorphic(&a, &b));
    assert!(!is_isomorphic(&a, &b));
}

#[bench]
fn praust_undir_no_iso_bench(bench: &mut Bencher) {
    let a = ungraph().praust_a();
    let b = ungraph().praust_b();

    bench.iter(|| is_isomorphic(&a, &b));
    assert!(!is_isomorphic(&a, &b));
}
