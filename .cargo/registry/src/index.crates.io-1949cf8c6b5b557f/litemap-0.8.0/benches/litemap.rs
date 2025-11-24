// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use litemap::LiteMap;
use rand::seq::SliceRandom;

const DATA: [(&str, &str); 16] = [
    ("ar", "Arabic"),
    ("bn", "Bangla"),
    ("ccp", "Chakma"),
    ("chr", "Cherokee"),
    ("el", "Greek"),
    ("en", "English"),
    ("eo", "Esperanto"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("iu", "Inuktitut"),
    ("ja", "Japanese"),
    ("ru", "Russian"),
    ("sr", "Serbian"),
    ("th", "Thai"),
    ("tr", "Turkish"),
    ("zh", "Chinese"),
];

const POSTCARD: [u8; 176] = [
    16, 2, 97, 114, 6, 65, 114, 97, 98, 105, 99, 2, 98, 110, 6, 66, 97, 110, 103, 108, 97, 3, 99,
    99, 112, 6, 67, 104, 97, 107, 109, 97, 3, 99, 104, 114, 8, 67, 104, 101, 114, 111, 107, 101,
    101, 2, 101, 108, 5, 71, 114, 101, 101, 107, 2, 101, 110, 7, 69, 110, 103, 108, 105, 115, 104,
    2, 101, 111, 9, 69, 115, 112, 101, 114, 97, 110, 116, 111, 2, 101, 115, 7, 83, 112, 97, 110,
    105, 115, 104, 2, 102, 114, 6, 70, 114, 101, 110, 99, 104, 2, 105, 117, 9, 73, 110, 117, 107,
    116, 105, 116, 117, 116, 2, 106, 97, 8, 74, 97, 112, 97, 110, 101, 115, 101, 2, 114, 117, 7,
    82, 117, 115, 115, 105, 97, 110, 2, 115, 114, 7, 83, 101, 114, 98, 105, 97, 110, 2, 116, 104,
    4, 84, 104, 97, 105, 2, 116, 114, 7, 84, 117, 114, 107, 105, 115, 104, 2, 122, 104, 7, 67, 104,
    105, 110, 101, 115, 101,
];

/// Run this function to print new data to the console.
#[allow(dead_code)]
fn generate() {
    let map = build_litemap(false);
    let buf = postcard::to_stdvec(&map).unwrap();
    println!("{buf:?}");
}

fn large_litemap_postcard_bytes() -> Vec<u8> {
    postcard::to_stdvec(&build_litemap(true)).unwrap()
}

fn overview_bench(c: &mut Criterion) {
    // Uncomment the following line to re-generate the binary data.
    // generate();

    bench_deserialize(c);
    bench_deserialize_large(c);
    bench_lookup(c);
    bench_lookup_large(c);
    bench_from_iter(c);
    bench_from_iter_rand_large(c);
    bench_from_iter_sorted(c);
    bench_from_iter_large_sorted(c);
    bench_extend_rand(c);
    bench_extend_rand_dups(c);
    bench_extend_from_litemap_rand(c);
}

fn build_litemap(large: bool) -> LiteMap<String, String> {
    let mut map: LiteMap<String, String> = LiteMap::new();
    for (key, value) in DATA.into_iter() {
        if large {
            for n in 0..8192 {
                map.insert(format!("{key}{n}"), value.to_owned());
            }
        } else {
            map.insert(key.to_owned(), value.to_owned());
        }
    }
    map
}

fn bench_deserialize(c: &mut Criterion) {
    c.bench_function("litemap/deserialize/small", |b| {
        b.iter(|| {
            let map: LiteMap<String, String> = postcard::from_bytes(black_box(&POSTCARD)).unwrap();
            assert_eq!(map.get("iu"), Some(&"Inuktitut".to_owned()));
        })
    });
}

fn bench_deserialize_large(c: &mut Criterion) {
    let buf = large_litemap_postcard_bytes();
    c.bench_function("litemap/deseralize/large", |b| {
        b.iter(|| {
            let map: LiteMap<String, String> = postcard::from_bytes(black_box(&buf)).unwrap();
            assert_eq!(map.get("iu3333"), Some(&"Inuktitut".to_owned()));
        });
    });
}

fn bench_from_iter(c: &mut Criterion) {
    c.bench_function("litemap/from_iter_rand/small", |b| {
        let mut ff = build_litemap(false).into_iter().collect::<Vec<_>>();
        ff[..].shuffle(&mut rand::rng());
        b.iter(|| {
            let map: LiteMap<&String, &String> = LiteMap::from_iter(ff.iter().map(|(k, v)| (k, v)));
            black_box(map)
        })
    });
}

fn bench_from_iter_rand_large(c: &mut Criterion) {
    c.bench_function("litemap/from_iter_rand/large", |b| {
        let mut ff = build_litemap(true).into_iter().collect::<Vec<_>>();
        ff[..].shuffle(&mut rand::rng());
        b.iter(|| {
            let map: LiteMap<&String, &String> = LiteMap::from_iter(ff.iter().map(|(k, v)| (k, v)));
            black_box(map)
        })
    });
}

fn bench_from_iter_sorted(c: &mut Criterion) {
    c.bench_function("litemap/from_iter_sorted/small", |b| {
        let ff = build_litemap(false).into_iter().collect::<Vec<_>>();
        b.iter(|| {
            let map: LiteMap<&String, &String> = LiteMap::from_iter(ff.iter().map(|(k, v)| (k, v)));
            black_box(map)
        })
    });
}

fn bench_from_iter_large_sorted(c: &mut Criterion) {
    c.bench_function("litemap/from_iter_sorted/large", |b| {
        let ff = build_litemap(true).into_iter().collect::<Vec<_>>();
        b.iter(|| {
            let map: LiteMap<&String, &String> = LiteMap::from_iter(ff.iter().map(|(k, v)| (k, v)));
            black_box(map)
        })
    });
}

fn bench_extend_rand(c: &mut Criterion) {
    c.bench_function("litemap/extend_rand/large", |b| {
        let mut ff = build_litemap(true).into_iter().collect::<Vec<_>>();
        ff[..].shuffle(&mut rand::rng());
        b.iter(|| {
            let mut map: LiteMap<&String, &String> = LiteMap::with_capacity(0);
            let mut iter = ff.iter().map(|(k, v)| (k, v));
            let step = ff.len().div_ceil(10);
            for _ in 0..10 {
                map.extend(iter.by_ref().take(step));
            }
            black_box(map)
        })
    });
}

fn bench_extend_rand_dups(c: &mut Criterion) {
    c.bench_function("litemap/extend_rand_dups/large", |b| {
        let mut ff = build_litemap(true).into_iter().collect::<Vec<_>>();
        ff[..].shuffle(&mut rand::rng());
        b.iter(|| {
            let mut map: LiteMap<&String, &String> = LiteMap::with_capacity(0);
            for _ in 0..2 {
                let mut iter = ff.iter().map(|(k, v)| (k, v));
                let step = ff.len().div_ceil(10);
                for _ in 0..10 {
                    map.extend(iter.by_ref().take(step));
                }
            }
            black_box(map)
        })
    });
}

fn bench_extend_from_litemap_rand(c: &mut Criterion) {
    c.bench_function("litemap/extend_from_litemap_rand/large", |b| {
        let mut ff = build_litemap(true).into_iter().collect::<Vec<_>>();
        ff[..].shuffle(&mut rand::rng());
        b.iter(|| {
            let mut map: LiteMap<&String, &String> = LiteMap::with_capacity(0);
            let mut iter = ff.iter().map(|(k, v)| (k, v));
            let step = ff.len().div_ceil(10);
            for _ in 0..10 {
                let tmp: LiteMap<&String, &String> = LiteMap::from_iter(iter.by_ref().take(step));
                map.extend_from_litemap(tmp);
            }
            black_box(map)
        })
    });
}

fn bench_lookup(c: &mut Criterion) {
    let map: LiteMap<String, String> = postcard::from_bytes(&POSTCARD).unwrap();
    c.bench_function("litemap/lookup/small", |b| {
        b.iter(|| {
            assert_eq!(map.get(black_box("iu")), Some(&"Inuktitut".to_owned()));
            assert_eq!(map.get(black_box("zz")), None);
        });
    });
}

fn bench_lookup_large(c: &mut Criterion) {
    let buf = large_litemap_postcard_bytes();
    let map: LiteMap<String, String> = postcard::from_bytes(&buf).unwrap();
    c.bench_function("litemap/lookup/large", |b| {
        b.iter(|| {
            assert_eq!(map.get(black_box("iu3333")), Some(&"Inuktitut".to_owned()));
            assert_eq!(map.get(black_box("zz")), None);
        });
    });
}

criterion_group!(benches, overview_bench);
criterion_main!(benches);
