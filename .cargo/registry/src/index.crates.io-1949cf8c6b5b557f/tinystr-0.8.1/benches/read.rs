// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

mod common;
use common::*;

use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Bencher;
use criterion::Criterion;

use tinystr::TinyAsciiStr;

fn read(c: &mut Criterion) {
    macro_rules! cfs {
        ($r:ty, $inputs:expr) => {
            |b: &mut Bencher| {
                let parsed: Vec<$r> = $inputs.iter().map(|s| s.parse().unwrap()).collect();
                b.iter(|| {
                    for s in &parsed {
                        let _: &str = black_box(&**s);
                    }
                })
            }
        };
    }

    bench_block!(c, "read", cfs);
}

criterion_group!(benches, read,);
criterion_main!(benches);
