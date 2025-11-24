#![feature(test)]
extern crate test;
extern crate fnv;
extern crate fxhash;
extern crate seahash;

use std::hash::{Hash, Hasher};
use test::{Bencher, black_box};


fn fnvhash<H: Hash>(b: H) -> u64 {
    let mut hasher = fnv::FnvHasher::default();
    b.hash(&mut hasher);
    hasher.finish()
}

fn seahash<H: Hash>(b: H) -> u64 {
    let mut hasher = seahash::SeaHasher::default();
    b.hash(&mut hasher);
    hasher.finish()
}

macro_rules! generate_benches {
    ($($fx:ident, $fx32:ident, $fx64:ident, $fnv:ident, $sea:ident, $s:expr),* $(,)*) => (
        $(
            #[bench]
            fn $fx(b: &mut Bencher) {
                let s = black_box($s);
                b.iter(|| {
                    fxhash::hash(&s)
                })
            }

            #[bench]
            fn $fx32(b: &mut Bencher) {
                let s = black_box($s);
                b.iter(|| {
                    fxhash::hash32(&s)
                })
            }

            #[bench]
            fn $fx64(b: &mut Bencher) {
                let s = black_box($s);
                b.iter(|| {
                    fxhash::hash64(&s)
                })
            }

            #[bench]
            fn $fnv(b: &mut Bencher) {
                let s = black_box($s);
                b.iter(|| {
                    fnvhash(&s)
                })
            }

            #[bench]
            fn $sea(b: &mut Bencher) {
                let s = black_box($s);
                b.iter(|| {
                    seahash(&s)
                })
            }
        )*
    )
}

generate_benches!(
    bench_fx_003, bench_fx32_003, bench_fx64_003, bench_fnv_003, bench_seahash_003, "123",
    bench_fx_004, bench_fx32_004, bench_fx64_004, bench_fnv_004, bench_seahash_004, "1234",
    bench_fx_011, bench_fx32_011, bench_fx64_011, bench_fnv_011, bench_seahash_011, "12345678901",
    bench_fx_012, bench_fx32_012, bench_fx64_012, bench_fnv_012, bench_seahash_012, "123456789012",
    bench_fx_023, bench_fx32_023, bench_fx64_023, bench_fnv_023, bench_seahash_023, "12345678901234567890123",
    bench_fx_024, bench_fx32_024, bench_fx64_024, bench_fnv_024, bench_seahash_024, "123456789012345678901234",
    bench_fx_068, bench_fx32_068, bench_fx64_068, bench_fnv_068, bench_seahash_068, "11234567890123456789012345678901234567890123456789012345678901234567",
    bench_fx_132, bench_fx32_132, bench_fx64_132, bench_fnv_132, bench_seahash_132, "112345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901",
);