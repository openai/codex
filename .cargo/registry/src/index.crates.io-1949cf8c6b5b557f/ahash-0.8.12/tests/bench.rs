#![cfg_attr(specialize, feature(build_hasher_simple_hash_one))]

use ahash::{AHasher, RandomState};
use criterion::*;
use fxhash::FxHasher;
use rand::Rng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{BuildHasherDefault, Hash, Hasher};

// Needs to be in sync with `src/lib.rs`
const AHASH_IMPL: &str = if cfg!(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "aes",
        not(miri),
    ),
    all(feature = "nightly-arm-aes", target_arch = "aarch64", target_feature = "aes", not(miri)),
    all(
        feature = "nightly-arm-aes",
        target_arch = "arm",
        target_feature = "aes",
        not(miri)
    ),
)) {
    "aeshash"
} else {
    "fallbackhash"
};

fn ahash<H: Hash>(b: &H) -> u64 {
    let build_hasher = RandomState::with_seeds(1, 2, 3, 4);
    build_hasher.hash_one(b)
}

fn fnvhash<H: Hash>(b: &H) -> u64 {
    let mut hasher = fnv::FnvHasher::default();
    b.hash(&mut hasher);
    hasher.finish()
}

fn siphash<H: Hash>(b: &H) -> u64 {
    let mut hasher = DefaultHasher::default();
    b.hash(&mut hasher);
    hasher.finish()
}

fn fxhash<H: Hash>(b: &H) -> u64 {
    let mut hasher = FxHasher::default();
    b.hash(&mut hasher);
    hasher.finish()
}

fn seahash<H: Hash>(b: &H) -> u64 {
    let mut hasher = seahash::SeaHasher::default();
    b.hash(&mut hasher);
    hasher.finish()
}

const STRING_LENGTHS: [u32; 12] = [1, 3, 4, 7, 8, 15, 16, 24, 33, 68, 132, 1024];

fn gen_strings() -> Vec<String> {
    STRING_LENGTHS
        .iter()
        .map(|len| {
            let mut string = String::default();
            for pos in 1..=*len {
                let c = (48 + (pos % 10) as u8) as char;
                string.push(c);
            }
            string
        })
        .collect()
}

macro_rules! bench_inputs {
    ($group:ident, $hash:ident) => {
        // Number of iterations per batch should be high enough to hide timing overhead.
        let size = BatchSize::NumIterations(50_000);

        let mut rng = rand::thread_rng();
        $group.bench_function("u8", |b| b.iter_batched(|| rng.gen::<u8>(), |v| $hash(&v), size));
        $group.bench_function("u16", |b| b.iter_batched(|| rng.gen::<u16>(), |v| $hash(&v), size));
        $group.bench_function("u32", |b| b.iter_batched(|| rng.gen::<u32>(), |v| $hash(&v), size));
        $group.bench_function("u64", |b| b.iter_batched(|| rng.gen::<u64>(), |v| $hash(&v), size));
        $group.bench_function("u128", |b| b.iter_batched(|| rng.gen::<u128>(), |v| $hash(&v), size));
        $group.bench_with_input("strings", &gen_strings(), |b, s| b.iter(|| $hash(black_box(s))));
    };
}

fn bench_ahash(c: &mut Criterion) {
    let mut group = c.benchmark_group(AHASH_IMPL);
    bench_inputs!(group, ahash);
}

fn bench_fx(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx");
    bench_inputs!(group, fxhash);
}

fn bench_fnv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fnv");
    bench_inputs!(group, fnvhash);
}

fn bench_sea(c: &mut Criterion) {
    let mut group = c.benchmark_group("sea");
    bench_inputs!(group, seahash);
}

fn bench_sip(c: &mut Criterion) {
    let mut group = c.benchmark_group("sip");
    bench_inputs!(group, siphash);
}

fn bench_map(c: &mut Criterion) {
    #[cfg(feature = "std")]
    {
        let mut group = c.benchmark_group("map");
        group.bench_function("aHash-alias", |b| {
            b.iter(|| {
                let hm: ahash::HashMap<i32, i32> = (0..1_000_000).map(|i| (i, i)).collect();
                let mut sum = 0;
                for i in 0..1_000_000 {
                    if let Some(x) = hm.get(&i) {
                        sum += x;
                    }
                }
            })
        });
        group.bench_function("aHash-hashBrown", |b| {
            b.iter(|| {
                let hm: hashbrown::HashMap<i32, i32> = (0..1_000_000).map(|i| (i, i)).collect();
                let mut sum = 0;
                for i in 0..1_000_000 {
                    if let Some(x) = hm.get(&i) {
                        sum += x;
                    }
                }
            })
        });
        group.bench_function("aHash-hashBrown-explicit", |b| {
            b.iter(|| {
                let hm: hashbrown::HashMap<i32, i32, RandomState> = (0..1_000_000).map(|i| (i, i)).collect();
                let mut sum = 0;
                for i in 0..1_000_000 {
                    if let Some(x) = hm.get(&i) {
                        sum += x;
                    }
                }
            })
        });
        group.bench_function("aHash-wrapper", |b| {
            b.iter(|| {
                let hm: ahash::AHashMap<i32, i32> = (0..1_000_000).map(|i| (i, i)).collect();
                let mut sum = 0;
                for i in 0..1_000_000 {
                    if let Some(x) = hm.get(&i) {
                        sum += x;
                    }
                }
            })
        });
        group.bench_function("aHash-rand", |b| {
            b.iter(|| {
                let hm: std::collections::HashMap<i32, i32, RandomState> = (0..1_000_000).map(|i| (i, i)).collect();
                let mut sum = 0;
                for i in 0..1_000_000 {
                    if let Some(x) = hm.get(&i) {
                        sum += x;
                    }
                }
            })
        });
        group.bench_function("aHash-default", |b| {
            b.iter(|| {
                let hm: std::collections::HashMap<i32, i32, BuildHasherDefault<AHasher>> =
                    (0..1_000_000).map(|i| (i, i)).collect();
                let mut sum = 0;
                for i in 0..1_000_000 {
                    if let Some(x) = hm.get(&i) {
                        sum += x;
                    }
                }
            })
        });
    }
}

criterion_main!(benches);

criterion_group!(
    benches,
    bench_ahash,
    bench_fx,
    bench_fnv,
    bench_sea,
    bench_sip,
    bench_map
);
