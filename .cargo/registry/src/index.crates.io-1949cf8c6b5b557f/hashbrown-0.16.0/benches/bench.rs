// This benchmark suite contains some benchmarks along a set of dimensions:
//   Hasher: std default (SipHash) and crate default (foldhash).
//   Int key distribution: low bit heavy, top bit heavy, and random.
//   Task: basic functionality: insert, insert_erase, lookup, lookup_fail, iter
#![feature(test)]

extern crate test;

use test::{black_box, Bencher};

use hashbrown::DefaultHashBuilder;
use hashbrown::{HashMap, HashSet};
use std::{
    collections::hash_map::RandomState,
    sync::atomic::{self, AtomicUsize},
};

const SIZE: usize = 1000;
const OP_COUNT: usize = 500;

// The default hashmap when using this crate directly.
type FoldHashMap<K, V> = HashMap<K, V, DefaultHashBuilder>;
// This uses the hashmap from this crate with the default hasher of the stdlib.
type StdHashMap<K, V> = HashMap<K, V, RandomState>;

// A random key iterator.
#[derive(Clone, Copy)]
struct RandomKeys {
    state: usize,
}

impl RandomKeys {
    fn new() -> Self {
        RandomKeys { state: 0 }
    }
}

impl Iterator for RandomKeys {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        // Add 1 then multiply by some 32 bit prime.
        self.state = self.state.wrapping_add(1).wrapping_mul(3_787_392_781);
        Some(self.state)
    }
}

// Just an arbitrary side effect to make the maps not shortcircuit to the non-dropping path
// when dropping maps/entries (most real world usages likely have drop in the key or value)
static SIDE_EFFECT: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
struct DropType(usize);
impl Drop for DropType {
    fn drop(&mut self) {
        SIDE_EFFECT.fetch_add(self.0, atomic::Ordering::SeqCst);
    }
}

macro_rules! bench_suite {
    ($bench_macro:ident, $bench_foldhash_serial:ident, $bench_std_serial:ident,
     $bench_foldhash_highbits:ident, $bench_std_highbits:ident,
     $bench_foldhash_random:ident, $bench_std_random:ident) => {
        $bench_macro!($bench_foldhash_serial, FoldHashMap, 0..);
        $bench_macro!($bench_std_serial, StdHashMap, 0..);
        $bench_macro!(
            $bench_foldhash_highbits,
            FoldHashMap,
            (0..).map(usize::swap_bytes)
        );
        $bench_macro!(
            $bench_std_highbits,
            StdHashMap,
            (0..).map(usize::swap_bytes)
        );
        $bench_macro!($bench_foldhash_random, FoldHashMap, RandomKeys::new());
        $bench_macro!($bench_std_random, StdHashMap, RandomKeys::new());
    };
}

macro_rules! bench_suite_2 {
    ($bench_macro:ident,
     $name0:ident, $size0:literal, $name1:ident, $size1:literal, $name2:ident, $size2:literal,
     $name3:ident, $size3:literal, $name4:ident, $size4:literal, $name5:ident, $size5:literal,
     $name6:ident, $size6:literal, $name7:ident, $size7:literal) => {
        $bench_macro!($name0, $size0, FoldHashMap, RandomKeys::new());
        $bench_macro!($name1, $size1, FoldHashMap, RandomKeys::new());
        $bench_macro!($name2, $size2, FoldHashMap, RandomKeys::new());
        $bench_macro!($name3, $size3, FoldHashMap, RandomKeys::new());
        $bench_macro!($name4, $size4, FoldHashMap, RandomKeys::new());
        $bench_macro!($name5, $size5, FoldHashMap, RandomKeys::new());
        $bench_macro!($name6, $size6, FoldHashMap, RandomKeys::new());
        $bench_macro!($name7, $size7, FoldHashMap, RandomKeys::new());
    };
}

macro_rules! bench_insert {
    ($name:ident, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut m = $maptype::with_capacity_and_hasher(SIZE, Default::default());
            b.iter(|| {
                m.clear();
                for i in ($keydist).take(SIZE) {
                    m.insert(i, (DropType(i), [i; 20]));
                }
                black_box(&mut m);
            });
            eprintln!("{}", SIDE_EFFECT.load(atomic::Ordering::SeqCst));
        }
    };
}

bench_suite!(
    bench_insert,
    insert_foldhash_serial,
    insert_std_serial,
    insert_foldhash_highbits,
    insert_std_highbits,
    insert_foldhash_random,
    insert_std_random
);

macro_rules! bench_grow_insert {
    ($name:ident, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            b.iter(|| {
                let mut m = $maptype::default();
                for i in ($keydist).take(SIZE) {
                    m.insert(i, DropType(i));
                }
                black_box(&mut m);
            })
        }
    };
}

bench_suite!(
    bench_grow_insert,
    grow_insert_foldhash_serial,
    grow_insert_std_serial,
    grow_insert_foldhash_highbits,
    grow_insert_std_highbits,
    grow_insert_foldhash_random,
    grow_insert_std_random
);

macro_rules! bench_insert_erase {
    ($name:ident, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut base = $maptype::default();
            for i in ($keydist).take(SIZE) {
                base.insert(i, DropType(i));
            }
            let skip = $keydist.skip(SIZE);
            b.iter(|| {
                let mut m = base.clone();
                let mut add_iter = skip.clone();
                let mut remove_iter = $keydist;
                // While keeping the size constant,
                // replace the first keydist with the second.
                for (add, remove) in (&mut add_iter).zip(&mut remove_iter).take(SIZE) {
                    m.insert(add, DropType(add));
                    black_box(m.remove(&remove));
                }
                black_box(m);
            });
            eprintln!("{}", SIDE_EFFECT.load(atomic::Ordering::SeqCst));
        }
    };
}

bench_suite!(
    bench_insert_erase,
    insert_erase_foldhash_serial,
    insert_erase_std_serial,
    insert_erase_foldhash_highbits,
    insert_erase_std_highbits,
    insert_erase_foldhash_random,
    insert_erase_std_random
);

macro_rules! bench_lookup {
    ($name:ident, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut m = $maptype::default();
            for i in $keydist.take(SIZE) {
                m.insert(i, DropType(i));
            }

            b.iter(|| {
                for i in $keydist.take(SIZE) {
                    black_box(m.get(&i));
                }
            });
            eprintln!("{}", SIDE_EFFECT.load(atomic::Ordering::SeqCst));
        }
    };
}

bench_suite!(
    bench_lookup,
    lookup_foldhash_serial,
    lookup_std_serial,
    lookup_foldhash_highbits,
    lookup_std_highbits,
    lookup_foldhash_random,
    lookup_std_random
);

macro_rules! bench_lookup_fail {
    ($name:ident, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut m = $maptype::default();
            let mut iter = $keydist;
            for i in (&mut iter).take(SIZE) {
                m.insert(i, DropType(i));
            }

            b.iter(|| {
                for i in (&mut iter).take(SIZE) {
                    black_box(m.get(&i));
                }
            })
        }
    };
}

bench_suite!(
    bench_lookup_fail,
    lookup_fail_foldhash_serial,
    lookup_fail_std_serial,
    lookup_fail_foldhash_highbits,
    lookup_fail_std_highbits,
    lookup_fail_foldhash_random,
    lookup_fail_std_random
);

macro_rules! bench_lookup_load_factor {
    ($name:ident, $size:literal, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut m = $maptype::default();
            for i in $keydist.take($size) {
                m.insert(i, DropType(i));
            }

            b.iter(|| {
                for i in $keydist.take(OP_COUNT) {
                    black_box(m.get(&i));
                }
            });
        }
    };
}

bench_suite_2!(
    bench_lookup_load_factor, // same capacity of 32768 * 0.875
    loadfactor_lookup_14500,
    14500,
    loadfactor_lookup_16500,
    16500,
    loadfactor_lookup_18500,
    18500,
    loadfactor_lookup_20500,
    20500,
    loadfactor_lookup_22500,
    22500,
    loadfactor_lookup_24500,
    24500,
    loadfactor_lookup_26500,
    26500,
    loadfactor_lookup_28500,
    28500
);

macro_rules! bench_lookup_fail_load_factor {
    ($name:ident, $size:literal, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut m = $maptype::default();
            let mut iter = $keydist;
            for i in (&mut iter).take($size) {
                m.insert(i, DropType(i));
            }

            b.iter(|| {
                for i in (&mut iter).take(OP_COUNT) {
                    black_box(m.get(&i));
                }
            })
        }
    };
}

bench_suite_2!(
    bench_lookup_fail_load_factor, // same capacity of 32768 * 0.875
    loadfactor_lookup_fail_14500,
    14500,
    loadfactor_lookup_fail_16500,
    16500,
    loadfactor_lookup_fail_18500,
    18500,
    loadfactor_lookup_fail_20500,
    20500,
    loadfactor_lookup_fail_22500,
    22500,
    loadfactor_lookup_fail_24500,
    24500,
    loadfactor_lookup_fail_26500,
    26500,
    loadfactor_lookup_fail_28500,
    28500
);

macro_rules! bench_iter {
    ($name:ident, $maptype:ident, $keydist:expr) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let mut m = $maptype::default();
            for i in ($keydist).take(SIZE) {
                m.insert(i, DropType(i));
            }

            b.iter(|| {
                for i in &m {
                    black_box(i);
                }
            })
        }
    };
}

bench_suite!(
    bench_iter,
    iter_foldhash_serial,
    iter_std_serial,
    iter_foldhash_highbits,
    iter_std_highbits,
    iter_foldhash_random,
    iter_std_random
);

#[bench]
fn clone_small(b: &mut Bencher) {
    let mut m = HashMap::new();
    for i in 0..10 {
        m.insert(i, DropType(i));
    }

    b.iter(|| {
        black_box(m.clone());
    })
}

#[bench]
fn clone_from_small(b: &mut Bencher) {
    let mut m = HashMap::new();
    let mut m2 = HashMap::new();
    for i in 0..10 {
        m.insert(i, DropType(i));
    }

    b.iter(|| {
        m2.clone_from(&m);
        black_box(&mut m2);
    })
}

#[bench]
fn clone_large(b: &mut Bencher) {
    let mut m = HashMap::new();
    for i in 0..1000 {
        m.insert(i, DropType(i));
    }

    b.iter(|| {
        black_box(m.clone());
    })
}

#[bench]
fn clone_from_large(b: &mut Bencher) {
    let mut m = HashMap::new();
    let mut m2 = HashMap::new();
    for i in 0..1000 {
        m.insert(i, DropType(i));
    }

    b.iter(|| {
        m2.clone_from(&m);
        black_box(&mut m2);
    })
}

#[bench]
fn rehash_in_place(b: &mut Bencher) {
    b.iter(|| {
        let mut set = HashSet::new();

        // Each loop triggers one rehash
        for _ in 0..10 {
            for i in 0..223 {
                set.insert(i);
            }

            assert_eq!(
                set.capacity(),
                224,
                "The set must be at or close to capacity to trigger a re hashing"
            );

            for i in 100..1400 {
                set.remove(&(i - 100));
                set.insert(i);
            }
            set.clear();
        }
    });
}
