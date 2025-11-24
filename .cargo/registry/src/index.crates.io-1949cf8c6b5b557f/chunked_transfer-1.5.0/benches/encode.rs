#![feature(test)]

use chunked_transfer;
use criterion::{criterion_group, criterion_main, Criterion};
use std::io::Write;

extern crate test;

fn encode_benchmark(c: &mut Criterion) {
    c.bench_function("encode", |b| {
        let writer = vec![];
        let mut encoder = chunked_transfer::Encoder::new(writer);
        let mut to_write = vec![b'a'; 1000];

        b.iter(|| {
            test::black_box(encoder.write_all(&mut to_write));
        });
    });
}

criterion_group!(benches, encode_benchmark);
criterion_main!(benches);
