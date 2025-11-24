// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::SeedableRng;
use rand_distr::{Alphanumeric, Distribution, Uniform};
use rand_pcg::Lcg64Xsh32;
use std::ops::RangeInclusive;

use zerovec::VarZeroVec;

/// Generates an array of random alphanumeric strings.
///
/// - length = range of lengths for the strings (chosen uniformly at random)
/// - count = number of strings to generate
/// - seed = seed for the PRNG
///
/// Returns a tuple including the vector and a u64 that can be used to seed the next PRNG.
fn random_alphanums(lengths: RangeInclusive<usize>, count: usize, seed: u64) -> (Vec<String>, u64) {
    // Lcg64Xsh32 is a small, fast PRNG for reproducible benchmarks.
    let mut rng1 = Lcg64Xsh32::seed_from_u64(seed);
    let mut rng2 = Lcg64Xsh32::seed_from_u64(rand::Rng::random(&mut rng1));
    let alpha_dist = Alphanumeric;
    let len_dist = Uniform::try_from(lengths).expect("range out of bounds");
    let string_vec = len_dist
        .sample_iter(&mut rng1)
        .take(count)
        .map(|len| {
            (&alpha_dist)
                .sample_iter(&mut rng2)
                .take(len)
                .map(char::from)
                .collect::<String>()
        })
        .collect();
    (string_vec, rand::Rng::random(&mut rng1))
}

fn overview_bench(c: &mut Criterion) {
    // Same as vzv/char_count/vzv but with different inputs
    let seed = 42;
    let (string_vec, _) = random_alphanums(2..=10, 100, seed);
    let bytes: Vec<u8> = VarZeroVec::<str>::from(&string_vec).into_bytes();
    let vzv = VarZeroVec::<str>::parse_bytes(black_box(bytes.as_slice())).unwrap();

    c.bench_function("vzv/overview", |b| {
        b.iter(|| {
            black_box(&vzv)
                .iter()
                .fold(0, |sum, string| sum + string.chars().count())
        });
    });

    {
        char_count_benches(c);
        binary_search_benches(c);
        vzv_precompute_bench(c);
    }

    #[cfg(feature = "serde")]
    {
        serde_benches(c);
    }
}

fn char_count_benches(c: &mut Criterion) {
    let seed = 2021;
    let (string_vec, _) = random_alphanums(2..=20, 100, seed);
    let bytes: Vec<u8> = VarZeroVec::<str>::from(&string_vec).into_bytes();
    let vzv = VarZeroVec::<str>::parse_bytes(black_box(bytes.as_slice())).unwrap();

    // *** Count chars in vec of 100 strings ***
    c.bench_function("vzv/char_count/slice", |b| {
        b.iter(|| {
            black_box(&string_vec)
                .iter()
                .fold(0, |sum, string| sum + string.chars().count())
        });
    });

    // *** Count chars in vec of 100 strings ***
    c.bench_function("vzv/char_count/vzv", |b| {
        b.iter(|| {
            black_box(&vzv)
                .iter()
                .fold(0, |sum, string| sum + string.chars().count())
        });
    });
}

fn binary_search_benches(c: &mut Criterion) {
    let seed = 2021;
    let (string_vec, seed) = random_alphanums(2..=20, 500, seed);
    let (needles, _) = random_alphanums(2..=20, 10, seed);
    let bytes: Vec<u8> = VarZeroVec::<str>::from(&string_vec).into_bytes();
    let vzv = VarZeroVec::<str>::parse_bytes(black_box(bytes.as_slice())).unwrap();
    let single_needle = "lmnop".to_owned();

    // *** Binary search vec of 500 strings 10 times ***
    c.bench_function("vzv/binary_search/slice", |b| {
        b.iter(|| {
            black_box(&needles)
                .iter()
                .map(|needle| black_box(&string_vec).binary_search(needle))
                .filter(|r| r.is_ok())
                .count()
        });
    });

    // *** Binary search vec of 500 strings 10 times ***
    c.bench_function("vzv/binary_search/vzv", |b| {
        b.iter(|| {
            black_box(&needles)
                .iter()
                .map(|needle| black_box(&vzv).binary_search(needle))
                .filter(|r| r.is_ok())
                .count()
        });
    });

    c.bench_function("vzv/binary_search/single/slice", |b| {
        b.iter(|| black_box(&string_vec).binary_search(black_box(&single_needle)));
    });

    c.bench_function("vzv/binary_search/single/vzv", |b| {
        b.iter(|| black_box(&vzv).binary_search(black_box(&single_needle)));
    });
}

#[cfg(feature = "serde")]
fn serde_benches(c: &mut Criterion) {
    let seed = 2021;
    let (string_vec, _) = random_alphanums(2..=20, 100, seed);
    let bincode_vec = bincode::serialize(&string_vec).unwrap();
    let vzv: VarZeroVec<str> = VarZeroVec::from(&*string_vec);
    let bincode_vzv = bincode::serialize(&vzv).unwrap();

    // *** Deserialize vec of 100 strings ***
    c.bench_function("vzv/deserialize/string/vec_owned", |b| {
        b.iter(|| bincode::deserialize::<Vec<String>>(black_box(&bincode_vec)));
    });

    // *** Deserialize vec of 100 strings ***
    c.bench_function("vzv/deserialize/string/vec_borrowed", |b| {
        b.iter(|| bincode::deserialize::<Vec<&str>>(black_box(&bincode_vec)));
    });

    // *** Deserialize vec of 100 strings ***
    c.bench_function("vzv/deserialize/string/vzv", |b| {
        b.iter(|| bincode::deserialize::<VarZeroVec<str>>(black_box(&bincode_vzv)));
    });
}

// Testing differences between operating on slices with precomputed/non-precomputed indexing info
fn vzv_precompute_bench(c: &mut Criterion) {
    let seed = 2021;
    let (string_vec, seed) = random_alphanums(2..=20, 500, seed);
    let (needles, _) = random_alphanums(2..=20, 10, seed);
    let bytes: Vec<u8> = VarZeroVec::<str>::from(&string_vec).into_bytes();
    let vzv = VarZeroVec::<str>::parse_bytes(black_box(bytes.as_slice())).unwrap();
    let borrowed = vzv.as_components();
    let slice = vzv.as_slice();
    let single_needle = "lmnop";

    c.bench_function("vzv_precompute/get/precomputed", |b| {
        b.iter(|| black_box(&borrowed).get(100));
    });

    c.bench_function("vzv_precompute/get/slice", |b| {
        b.iter(|| black_box(&slice).get(100));
    });

    c.bench_function("vzv_precompute/search/precomputed", |b| {
        b.iter(|| black_box(&borrowed).binary_search(single_needle));
    });

    c.bench_function("vzv_precompute/search/slice", |b| {
        b.iter(|| black_box(&slice).binary_search(single_needle));
    });

    c.bench_function("vzv_precompute/search_multi/precomputed", |b| {
        b.iter(|| {
            black_box(&needles)
                .iter()
                .map(|needle| black_box(&borrowed).binary_search(needle))
                .filter(|r| r.is_ok())
                .count()
        });
    });

    c.bench_function("vzv_precompute/search_multi/slice", |b| {
        b.iter(|| {
            black_box(&needles)
                .iter()
                .map(|needle| black_box(&slice).binary_search(needle))
                .filter(|r| r.is_ok())
                .count()
        });
    });
}

criterion_group!(benches, overview_bench,);
criterion_main!(benches);
