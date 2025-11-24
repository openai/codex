//! Compares the performance of `UnicodeSegmentation::graphemes` with stdlib's UTF-8 scalar-based
//! `std::str::chars`.
//!
//! It is expected that `std::str::chars` is faster than `UnicodeSegmentation::graphemes` since it
//! does not consider the complexity of grapheme clusters. The question in this benchmark
//! is how much slower full unicode handling is.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use std::fs;
use unicode_segmentation::UnicodeSegmentation;

const FILES: &[&str] = &[
    "arabic",
    "english",
    "hindi",
    "japanese",
    "korean",
    "mandarin",
    "russian",
    "source_code",
];

#[inline(always)]
fn grapheme(text: &str) {
    for c in UnicodeSegmentation::graphemes(black_box(text), true) {
        black_box(c);
    }
}

#[inline(always)]
fn scalar(text: &str) {
    for c in black_box(text).chars() {
        black_box(c);
    }
}

fn bench_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("chars");

    for file in FILES {
        group.bench_with_input(
            BenchmarkId::new("grapheme", file),
            &fs::read_to_string(format!("benches/texts/{}.txt", file)).unwrap(),
            |b, content| b.iter(|| grapheme(content)),
        );
    }

    for file in FILES {
        group.bench_with_input(
            BenchmarkId::new("scalar", file),
            &fs::read_to_string(format!("benches/texts/{}.txt", file)).unwrap(),
            |b, content| b.iter(|| scalar(content)),
        );
    }
}

criterion_group!(benches, bench_all);
criterion_main!(benches);
