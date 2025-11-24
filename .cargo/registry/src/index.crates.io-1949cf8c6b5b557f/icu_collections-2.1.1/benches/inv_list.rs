// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{criterion_group, criterion_main, Criterion};
use icu_collections::codepointinvlist::CodePointInversionList;
use std::char;

fn uniset_bench(c: &mut Criterion) {
    let best_ex = [0x41, 0x46];
    let best_sample = CodePointInversionList::try_from_u32_inversion_list_slice(&best_ex).unwrap();
    let worst_ex: Vec<u32> = (0x0..((char::MAX as u32) + 1)).collect();
    let worst_sample =
        CodePointInversionList::try_from_u32_inversion_list_slice(&worst_ex).unwrap();

    c.bench_function("uniset/overview", |b| {
        #[expect(clippy::suspicious_map)]
        b.iter(|| {
            best_sample
                .iter_chars()
                .map(|ch| best_sample.contains(ch))
                .count();
            worst_sample
                .iter_chars()
                .map(|ch| worst_sample.contains(ch))
                .count();
            best_sample
                .iter_chars()
                .map(|ch| best_sample.contains_range('A'..ch))
                .count();
            worst_sample
                .iter_chars()
                .take(100)
                .map(|ch| worst_sample.contains_range(char::from_u32(0x0).unwrap()..ch))
                .count();
        })
    });

    {
        let mut group = c.benchmark_group("uniset/contains");
        group.bench_with_input("best", &best_sample, |b, sample| {
            b.iter(|| sample.iter_chars().map(|ch| sample.contains(ch)))
        });
        group.bench_with_input("worst", &worst_sample, |b, sample| {
            b.iter(|| sample.iter_chars().take(100).map(|ch| sample.contains(ch)))
        });
        group.finish();

        let mut group = c.benchmark_group("uniset/contains_range");
        group.bench_with_input("best", &best_sample, |b, sample| {
            b.iter(|| sample.iter_chars().map(|ch| sample.contains_range('A'..ch)))
        });
        group.bench_with_input("worst", &worst_sample, |b, sample| {
            b.iter(|| {
                sample
                    .iter_chars()
                    .take(100)
                    .map(|ch| sample.contains_range(char::from_u32(0x0).unwrap()..ch))
            })
        });
        group.finish();
    }
}

criterion_group!(benches, uniset_bench);
criterion_main!(benches);
