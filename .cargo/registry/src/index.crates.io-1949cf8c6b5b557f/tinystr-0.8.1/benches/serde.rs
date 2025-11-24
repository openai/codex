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

fn deserialize(c: &mut Criterion) {
    macro_rules! cfs {
        ($r:ty, $inputs:expr) => {
            |b: &mut Bencher| {
                let serialized: Vec<Vec<u8>> = $inputs
                    .iter()
                    .map(|s| postcard::to_stdvec(&s.parse::<$r>().unwrap()).unwrap())
                    .collect();
                b.iter(|| {
                    for bytes in &serialized {
                        let _: Result<$r, _> = black_box(postcard::from_bytes(bytes));
                    }
                })
            }
        };
    }

    bench_block!(c, "deserialize", cfs);
}

criterion_group!(benches, deserialize,);
criterion_main!(benches);
