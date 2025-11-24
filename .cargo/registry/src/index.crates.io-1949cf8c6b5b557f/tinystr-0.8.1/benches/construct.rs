// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// This file was adapted from https://github.com/zbraniecki/tinystr

mod common;
use common::*;

use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Bencher;
use criterion::Criterion;

use tinystr::TinyAsciiStr;

fn construct_from_str(c: &mut Criterion) {
    macro_rules! cfs {
        ($r:ty, $inputs:expr) => {
            |b: &mut Bencher| {
                b.iter(|| {
                    for s in $inputs {
                        let _: $r = black_box(s.parse().unwrap());
                    }
                })
            }
        };
    }

    bench_block!(c, "construct_from_str", cfs);
}

fn construct_from_utf8(c: &mut Criterion) {
    macro_rules! cfu {
        ($r:ty, $inputs:expr) => {
            |b| {
                let raw: Vec<&[u8]> = $inputs.iter().map(|s| s.as_bytes()).collect();
                b.iter(move || {
                    for u in &raw {
                        let _ = black_box(<$r>::try_from_utf8(*u).unwrap());
                    }
                })
            }
        };
    }

    let mut group4 = c.benchmark_group("construct_from_utf8/4");
    group4.bench_function("TinyAsciiStr<4>", cfu!(TinyAsciiStr<4>, STRINGS_4));
    group4.bench_function("TinyAsciiStr<8>", cfu!(TinyAsciiStr<8>, STRINGS_4));
    group4.bench_function("TinyAsciiStr<16>", cfu!(TinyAsciiStr<16>, STRINGS_4));
    group4.finish();

    let mut group8 = c.benchmark_group("construct_from_utf8/8");
    group8.bench_function("TinyAsciiStr<8>", cfu!(TinyAsciiStr<8>, STRINGS_8));
    group8.bench_function("TinyAsciiStr<16>", cfu!(TinyAsciiStr<16>, STRINGS_8));
    group8.finish();

    let mut group16 = c.benchmark_group("construct_from_utf8/16");
    group16.bench_function("TinyAsciiStr<16>", cfu!(TinyAsciiStr<16>, STRINGS_16));
    group16.finish();
}

criterion_group!(benches, construct_from_str, construct_from_utf8,);
criterion_main!(benches);
