// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::SeedableRng;
use rand_distr::{Distribution, LogNormal};
use rand_pcg::Lcg64Xsh32;

#[path = "../src/samples.rs"]
mod samples;
use samples::*;

use zerovec::ZeroVec;

/// Generate a large list of u32s for stress testing.
#[allow(dead_code)]
fn random_numbers(count: usize) -> Vec<u32> {
    // Lcg64Xsh32 is a small, fast PRNG for reproducible benchmarks.
    // LogNormal(10, 1) generates numbers with mean 36315 and mode 8103, a distribution that, in
    // spirit, correlates with Unicode properties (many low values and a long tail of high values)
    let mut rng = Lcg64Xsh32::seed_from_u64(2021);
    let dist = LogNormal::new(10.0, 1.0).unwrap();
    (&dist)
        .sample_iter(&mut rng)
        .take(count)
        .map(|f| f as u32)
        .collect()
}

fn overview_bench(c: &mut Criterion) {
    c.bench_function("zerovec_serde/overview", |b| {
        // Same as "zerovec_serde/deserialize_sum/u32/zerovec"
        let buffer =
            bincode::serialize(&ZeroVec::<u32>::parse_bytes(black_box(TEST_BUFFER_LE)).unwrap())
                .unwrap();
        b.iter(|| {
            bincode::deserialize::<ZeroVec<u32>>(&buffer)
                .unwrap()
                .iter()
                .sum::<u32>()
        });
    });

    {
        u32_benches(c);
        char_benches(c);
        stress_benches(c);
    }
}

fn u32_benches(c: &mut Criterion) {
    c.bench_function("zerovec_serde/serialize/u32/slice", |b| {
        b.iter(|| bincode::serialize(&Vec::from(black_box(TEST_SLICE))));
    });

    c.bench_function("zerovec_serde/deserialize_sum/u32/slice", |b| {
        let buffer = bincode::serialize(&Vec::from(black_box(TEST_SLICE))).unwrap();
        b.iter(|| {
            bincode::deserialize::<Vec<u32>>(&buffer)
                .unwrap()
                .iter()
                .sum::<u32>()
        });
    });

    c.bench_function("zerovec_serde/serialize/u32/zerovec", |b| {
        b.iter(|| bincode::serialize(&ZeroVec::from_slice_or_alloc(black_box(TEST_SLICE))));
    });

    c.bench_function("zerovec_serde/deserialize_sum/u32/zerovec", |b| {
        let buffer =
            bincode::serialize(&ZeroVec::<u32>::parse_bytes(black_box(TEST_BUFFER_LE)).unwrap())
                .unwrap();
        b.iter(|| {
            bincode::deserialize::<ZeroVec<u32>>(&buffer)
                .unwrap()
                .iter()
                .sum::<u32>()
        });
    });
}

fn char_benches(c: &mut Criterion) {
    const ORIGINAL_CHARS: &[char] = &[
        'ⶢ', '⺇', 'Ⱜ', '◁', '◩', '⌂', '⼅', '⏻', '⢜', '◊', 'ⲫ', '⏷', '◢', '⟉', '℞',
    ];

    let char_zero_vec = &ZeroVec::alloc_from_slice(ORIGINAL_CHARS);

    c.bench_function("zerovec_serde/serialize/char/slice", |b| {
        b.iter(|| bincode::serialize(black_box(&Vec::from(ORIGINAL_CHARS))));
    });

    c.bench_function("zerovec_serde/deserialize/char/slice", |b| {
        let buffer = bincode::serialize(black_box(&Vec::from(ORIGINAL_CHARS))).unwrap();
        b.iter(|| bincode::deserialize::<Vec<char>>(&buffer));
    });

    c.bench_function("zerovec_serde/serialize/char/zerovec", |b| {
        b.iter(|| bincode::serialize(black_box(char_zero_vec)));
    });

    c.bench_function("zerovec_serde/deserialize/char/zerovec", |b| {
        let buffer = bincode::serialize(black_box(char_zero_vec)).unwrap();
        b.iter(|| bincode::deserialize::<ZeroVec<char>>(&buffer));
    });
}

fn stress_benches(c: &mut Criterion) {
    let number_vec = random_numbers(100);
    let bincode_vec = bincode::serialize(&number_vec).unwrap();
    let zerovec_aligned = ZeroVec::from_slice_or_alloc(number_vec.as_slice());
    let bincode_zerovec = bincode::serialize(&zerovec_aligned).unwrap();

    // *** Deserialize vec of 100 `u32` ***
    c.bench_function("zerovec_serde/deserialize/stress/vec", |b| {
        b.iter(|| bincode::deserialize::<Vec<u32>>(&bincode_vec));
    });

    // *** Deserialize vec of 100 `u32` ***
    c.bench_function("zerovec_serde/deserialize/stress/zerovec", |b| {
        b.iter(|| bincode::deserialize::<ZeroVec<u32>>(&bincode_zerovec));
    });

    // *** Compute sum of vec of 100 `u32` ***
    c.bench_function("zerovec_serde/sum/stress/vec", |b| {
        b.iter(|| black_box(&number_vec).iter().sum::<u32>());
    });

    // *** Compute sum of vec of 100 `u32` ***
    let zerovec = ZeroVec::<u32>::parse_bytes(zerovec_aligned.as_bytes()).unwrap();
    c.bench_function("zerovec_serde/sum/stress/zerovec", |b| {
        b.iter(|| black_box(&zerovec).iter().sum::<u32>());
    });
}

criterion_group!(benches, overview_bench,);
criterion_main!(benches);
