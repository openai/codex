#![feature(test)]

extern crate petgraph;
extern crate test;

use test::Bencher;

use petgraph::algo::greedy_feedback_arc_set;

#[allow(dead_code)]
mod common;

use common::{directed_fan, tournament};

#[bench]
fn greedy_fas_tournament_10_bench(bench: &mut Bencher) {
    let g = tournament(10);

    bench.iter(|| greedy_feedback_arc_set(&g).for_each(|_| ()))
}

#[bench]
fn greedy_fas_tournament_50_bench(bench: &mut Bencher) {
    let g = tournament(50);

    bench.iter(|| greedy_feedback_arc_set(&g).for_each(|_| ()))
}

#[bench]
fn greedy_fas_tournament_200_bench(bench: &mut Bencher) {
    let g = tournament(200);

    bench.iter(|| greedy_feedback_arc_set(&g).for_each(|_| ()))
}

#[bench]
fn greedy_fas_fan_10_bench(bench: &mut Bencher) {
    let g = directed_fan(10);

    bench.iter(|| greedy_feedback_arc_set(&g).for_each(|_| ()))
}

#[bench]
fn greedy_fas_fan_200_bench(bench: &mut Bencher) {
    let g = directed_fan(200);

    bench.iter(|| greedy_feedback_arc_set(&g).for_each(|_| ()))
}

#[bench]
fn greedy_fas_fan_1000_bench(bench: &mut Bencher) {
    let g = directed_fan(1000);

    bench.iter(|| greedy_feedback_arc_set(&g).for_each(|_| ()))
}
