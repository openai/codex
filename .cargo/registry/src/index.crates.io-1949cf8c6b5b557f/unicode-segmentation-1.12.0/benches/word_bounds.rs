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
    for w in text.split_word_bounds() {
        black_box(w);
    }
}

fn bench_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("word_bounds");

    for file in FILES {
        group.bench_with_input(
            BenchmarkId::new("grapheme", file),
            &fs::read_to_string(format!("benches/texts/{}.txt", file)).unwrap(),
            |b, content| b.iter(|| grapheme(content)),
        );
    }
}

criterion_group!(benches, bench_all);
criterion_main!(benches);
