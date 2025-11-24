//! Compares the performance of `UnicodeSegmentation::unicode_words` with stdlib's UTF-8
//! scalar-based `std::str::split_whitespace`.
//!
//! It is expected that `std::str::split_whitespace` is faster than
//! `UnicodeSegmentation::unicode_words` since it does not consider the complexity of grapheme
//! clusters. The question in this benchmark is how much slower full unicode handling is.

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
    for w in text.unicode_words() {
        black_box(w);
    }
}

#[inline(always)]
fn scalar(text: &str) {
    for w in text.split_whitespace() {
        black_box(w);
    }
}

fn bench_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("words");

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
