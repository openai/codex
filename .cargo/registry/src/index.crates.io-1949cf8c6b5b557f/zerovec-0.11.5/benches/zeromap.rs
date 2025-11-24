// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use zerovec::maps::ZeroMapKV;
use zerovec::vecs::{Index32, VarZeroSlice, VarZeroVec};
use zerovec::{ZeroHashMap, ZeroMap};

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

const POSTCARD: [u8; 274] = [
    98, 16, 0, 0, 0, 2, 0, 0, 0, 4, 0, 0, 0, 7, 0, 0, 0, 10, 0, 0, 0, 12, 0, 0, 0, 14, 0, 0, 0, 16,
    0, 0, 0, 18, 0, 0, 0, 20, 0, 0, 0, 22, 0, 0, 0, 24, 0, 0, 0, 26, 0, 0, 0, 28, 0, 0, 0, 30, 0,
    0, 0, 32, 0, 0, 0, 97, 114, 98, 110, 99, 99, 112, 99, 104, 114, 101, 108, 101, 110, 101, 111,
    101, 115, 102, 114, 105, 117, 106, 97, 114, 117, 115, 114, 116, 104, 116, 114, 122, 104, 173,
    1, 16, 0, 0, 0, 6, 0, 0, 0, 12, 0, 0, 0, 18, 0, 0, 0, 26, 0, 0, 0, 31, 0, 0, 0, 38, 0, 0, 0,
    47, 0, 0, 0, 54, 0, 0, 0, 60, 0, 0, 0, 69, 0, 0, 0, 77, 0, 0, 0, 84, 0, 0, 0, 91, 0, 0, 0, 95,
    0, 0, 0, 102, 0, 0, 0, 65, 114, 97, 98, 105, 99, 66, 97, 110, 103, 108, 97, 67, 104, 97, 107,
    109, 97, 67, 104, 101, 114, 111, 107, 101, 101, 71, 114, 101, 101, 107, 69, 110, 103, 108, 105,
    115, 104, 69, 115, 112, 101, 114, 97, 110, 116, 111, 83, 112, 97, 110, 105, 115, 104, 70, 114,
    101, 110, 99, 104, 73, 110, 117, 107, 116, 105, 116, 117, 116, 74, 97, 112, 97, 110, 101, 115,
    101, 82, 117, 115, 115, 105, 97, 110, 83, 101, 114, 98, 105, 97, 110, 84, 104, 97, 105, 84,
    117, 114, 107, 105, 115, 104, 67, 104, 105, 110, 101, 115, 101,
];

const POSTCARD_HASHMAP: [u8; 176] = [
    16, 2, 114, 117, 7, 82, 117, 115, 115, 105, 97, 110, 3, 99, 99, 112, 6, 67, 104, 97, 107, 109,
    97, 3, 99, 104, 114, 8, 67, 104, 101, 114, 111, 107, 101, 101, 2, 116, 114, 7, 84, 117, 114,
    107, 105, 115, 104, 2, 116, 104, 4, 84, 104, 97, 105, 2, 106, 97, 8, 74, 97, 112, 97, 110, 101,
    115, 101, 2, 101, 115, 7, 83, 112, 97, 110, 105, 115, 104, 2, 101, 111, 9, 69, 115, 112, 101,
    114, 97, 110, 116, 111, 2, 122, 104, 7, 67, 104, 105, 110, 101, 115, 101, 2, 115, 114, 7, 83,
    101, 114, 98, 105, 97, 110, 2, 101, 110, 7, 69, 110, 103, 108, 105, 115, 104, 2, 105, 117, 9,
    73, 110, 117, 107, 116, 105, 116, 117, 116, 2, 102, 114, 6, 70, 114, 101, 110, 99, 104, 2, 98,
    110, 6, 66, 97, 110, 103, 108, 97, 2, 101, 108, 5, 71, 114, 101, 101, 107, 2, 97, 114, 6, 65,
    114, 97, 98, 105, 99,
];

const POSTCARD_ZEROHASHMAP: [u8; 404] = [
    128, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
    0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 98, 16, 0, 0, 0, 2, 0, 0, 0, 4, 0, 0, 0, 6, 0, 0, 0, 8, 0, 0, 0, 10, 0, 0, 0, 13, 0,
    0, 0, 15, 0, 0, 0, 17, 0, 0, 0, 19, 0, 0, 0, 21, 0, 0, 0, 24, 0, 0, 0, 26, 0, 0, 0, 28, 0, 0,
    0, 30, 0, 0, 0, 32, 0, 0, 0, 115, 114, 101, 111, 116, 114, 97, 114, 105, 117, 99, 99, 112, 102,
    114, 101, 115, 106, 97, 122, 104, 99, 104, 114, 98, 110, 101, 110, 101, 108, 114, 117, 116,
    104, 173, 1, 16, 0, 0, 0, 7, 0, 0, 0, 16, 0, 0, 0, 23, 0, 0, 0, 29, 0, 0, 0, 38, 0, 0, 0, 44,
    0, 0, 0, 50, 0, 0, 0, 57, 0, 0, 0, 65, 0, 0, 0, 72, 0, 0, 0, 80, 0, 0, 0, 86, 0, 0, 0, 93, 0,
    0, 0, 98, 0, 0, 0, 105, 0, 0, 0, 83, 101, 114, 98, 105, 97, 110, 69, 115, 112, 101, 114, 97,
    110, 116, 111, 84, 117, 114, 107, 105, 115, 104, 65, 114, 97, 98, 105, 99, 73, 110, 117, 107,
    116, 105, 116, 117, 116, 67, 104, 97, 107, 109, 97, 70, 114, 101, 110, 99, 104, 83, 112, 97,
    110, 105, 115, 104, 74, 97, 112, 97, 110, 101, 115, 101, 67, 104, 105, 110, 101, 115, 101, 67,
    104, 101, 114, 111, 107, 101, 101, 66, 97, 110, 103, 108, 97, 69, 110, 103, 108, 105, 115, 104,
    71, 114, 101, 101, 107, 82, 117, 115, 115, 105, 97, 110, 84, 104, 97, 105,
];

/// Run this function to print new data to the console.
/// Requires the optional `serde` Cargo feature.
#[allow(dead_code)]
fn generate_zeromap() {
    let map = build_zeromap(false);
    let buf = postcard::to_stdvec(&map).unwrap();
    println!("{buf:?}");
}

/// Run this function to print new data to the console.
/// Requires the optional `serde` Cargo feature.
#[allow(dead_code)]
fn generate_hashmap() {
    let map = build_hashmap(false);
    let buf = postcard::to_stdvec(&map).unwrap();
    println!("{buf:?}");
}

/// Run this function to print new data to the console.
/// Requires the optional `serde` Cargo feature.
#[allow(dead_code)]
fn generate_zerohashmap() {
    let map = build_zerohashmap(false);
    let buf = postcard::to_stdvec(&map).unwrap();
    println!("{buf:?}");
}

fn overview_bench(c: &mut Criterion) {
    bench_zeromap(c);
    bench_hashmap(c);
    bench_zerohashmap(c);
}

fn bench_zeromap(c: &mut Criterion) {
    // Uncomment the following line to re-generate the const data.
    // generate_hashmap();

    bench_deserialize(c);
    bench_deserialize_large(c);
    bench_lookup(c);
    bench_lookup_large(c);
}

fn build_zeromap(large: bool) -> ZeroMap<'static, Index32Str, Index32Str> {
    // TODO(#2826): This should use ZeroMap::from_iter, however that currently takes
    // *minutes*, whereas this code runs in milliseconds
    let mut keys = Vec::new();
    let mut values = Vec::new();
    let mut data = DATA.to_vec();
    data.sort();
    for &(key, value) in data.iter() {
        if large {
            for n in 0..8192 {
                keys.push(format!("{key}{n:04}"));
                values.push(indexify(value));
            }
        } else {
            keys.push(key.to_owned());
            values.push(indexify(value));
        }
    }

    let keys = keys.iter().map(|s| indexify(s)).collect::<Vec<_>>();
    // keys are sorted by construction
    unsafe { ZeroMap::from_parts_unchecked(VarZeroVec::from(&keys), VarZeroVec::from(&values)) }
}

fn bench_deserialize(c: &mut Criterion) {
    c.bench_function("zeromap/deserialize/small", |b| {
        b.iter(|| {
            let map: ZeroMap<Index32Str, Index32Str> =
                postcard::from_bytes(black_box(&POSTCARD)).unwrap();
            assert_eq!(map.get(indexify("iu")).map(|x| &x.0), Some("Inuktitut"));
        })
    });
}

fn bench_deserialize_large(c: &mut Criterion) {
    let buf = large_zeromap_postcard_bytes();
    c.bench_function("zeromap/deserialize/large", |b| {
        b.iter(|| {
            let map: ZeroMap<Index32Str, Index32Str> =
                postcard::from_bytes(black_box(&buf)).unwrap();
            assert_eq!(map.get(indexify("iu3333")).map(|x| &x.0), Some("Inuktitut"));
        })
    });
}

fn bench_lookup(c: &mut Criterion) {
    let map: ZeroMap<Index32Str, Index32Str> = postcard::from_bytes(black_box(&POSTCARD)).unwrap();
    c.bench_function("zeromap/lookup/small", |b| {
        b.iter(|| {
            assert_eq!(
                map.get(black_box(indexify("iu"))).map(|x| &x.0),
                Some("Inuktitut")
            );
            assert_eq!(map.get(black_box(indexify("zz"))).map(|x| &x.0), None);
        });
    });
}

fn bench_lookup_large(c: &mut Criterion) {
    let buf = large_zeromap_postcard_bytes();
    let map: ZeroMap<Index32Str, Index32Str> = postcard::from_bytes(&buf).unwrap();
    c.bench_function("zeromap/lookup/large", |b| {
        b.iter(|| {
            assert_eq!(
                map.get(black_box(indexify("iu3333"))).map(|x| &x.0),
                Some("Inuktitut")
            );
            assert_eq!(map.get(black_box(indexify("zz"))).map(|x| &x.0), None);
        });
    });
}

fn large_zeromap_postcard_bytes() -> Vec<u8> {
    postcard::to_stdvec(&build_zeromap(true)).unwrap()
}

fn bench_hashmap(c: &mut Criterion) {
    // Uncomment the following line to re-generate the const data.
    // generate_hashmap();

    bench_deserialize_hashmap(c);
    bench_deserialize_large_hashmap(c);
    bench_lookup_hashmap(c);
    bench_lookup_large_hashmap(c);
}

fn build_hashmap(large: bool) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for &(key, value) in DATA.iter() {
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

fn bench_deserialize_hashmap(c: &mut Criterion) {
    c.bench_function("zeromap/deserialize/small/hashmap", |b| {
        b.iter(|| {
            let map: HashMap<String, String> =
                postcard::from_bytes(black_box(&POSTCARD_HASHMAP)).unwrap();
            assert_eq!(map.get("iu"), Some(&"Inuktitut".to_owned()));
        })
    });
}

fn bench_deserialize_large_hashmap(c: &mut Criterion) {
    let buf = large_hashmap_postcard_bytes();
    c.bench_function("zeromap/deserialize/large/hashmap", |b| {
        b.iter(|| {
            let map: HashMap<String, String> = postcard::from_bytes(black_box(&buf)).unwrap();
            assert_eq!(map.get("iu3333"), Some(&"Inuktitut".to_owned()));
        })
    });
}

fn bench_lookup_hashmap(c: &mut Criterion) {
    let map: HashMap<String, String> = postcard::from_bytes(black_box(&POSTCARD_HASHMAP)).unwrap();
    c.bench_function("zeromap/lookup/small/hashmap", |b| {
        b.iter(|| {
            assert_eq!(map.get(black_box("iu")), Some(&"Inuktitut".to_owned()));
            assert_eq!(map.get(black_box("zz")), None);
        });
    });
}

fn bench_lookup_large_hashmap(c: &mut Criterion) {
    let buf = large_hashmap_postcard_bytes();
    let map: HashMap<String, String> = postcard::from_bytes(&buf).unwrap();
    c.bench_function("zeromap/lookup/large/hashmap", |b| {
        b.iter(|| {
            assert_eq!(map.get(black_box("iu3333")), Some(&"Inuktitut".to_owned()));
            assert_eq!(map.get(black_box("zz")), None);
        });
    });
}

fn large_hashmap_postcard_bytes() -> Vec<u8> {
    postcard::to_stdvec(&build_hashmap(true)).unwrap()
}

fn bench_zerohashmap(c: &mut Criterion) {
    // Uncomment the following line to re-generate the const data.
    // generate_zerohashmap();

    bench_deserialize_zerohashmap(c);
    bench_deserialize_large_zerohashmap(c);
    bench_zerohashmap_lookup(c);
    bench_zerohashmap_lookup_large(c);
}

fn build_zerohashmap(large: bool) -> ZeroHashMap<'static, Index32Str, Index32Str> {
    let mut kv = Vec::new();

    for (key, value) in DATA.iter() {
        if large {
            for n in 0..512 {
                kv.push((format!("{key}{n}"), indexify(value)));
            }
        } else {
            kv.push((key.to_string(), indexify(value)));
        }
    }

    ZeroHashMap::from_iter(kv.iter().map(|kv| (indexify(&kv.0), kv.1)))
}

fn bench_deserialize_zerohashmap(c: &mut Criterion) {
    c.bench_function("zerohashmap/deserialize/small", |b| {
        b.iter(|| {
            let map: ZeroHashMap<Index32Str, Index32Str> =
                postcard::from_bytes(black_box(&POSTCARD_ZEROHASHMAP)).unwrap();
            assert_eq!(map.get(indexify("iu")).map(|x| &x.0), Some("Inuktitut"));
        })
    });
}

fn bench_deserialize_large_zerohashmap(c: &mut Criterion) {
    let buf = large_zerohashmap_postcard_bytes();
    c.bench_function("zerohashmap/deserialize/large", |b| {
        b.iter(|| {
            let map: ZeroHashMap<Index32Str, Index32Str> =
                postcard::from_bytes(black_box(&buf)).unwrap();
            assert_eq!(map.get(indexify("iu333")).map(|x| &x.0), Some("Inuktitut"));
        })
    });
}

fn bench_zerohashmap_lookup(c: &mut Criterion) {
    let zero_hashmap: ZeroHashMap<Index32Str, Index32Str> =
        postcard::from_bytes(black_box(&POSTCARD_ZEROHASHMAP)).unwrap();

    c.bench_function("zerohashmap/lookup/small", |b| {
        b.iter(|| {
            assert_eq!(
                zero_hashmap.get(black_box(indexify("iu"))).map(|x| &x.0),
                Some("Inuktitut")
            );
            assert_eq!(
                zero_hashmap.get(black_box(indexify("zz"))).map(|x| &x.0),
                None
            );
        });
    });
}

fn bench_zerohashmap_lookup_large(c: &mut Criterion) {
    let buf = large_zerohashmap_postcard_bytes();
    let zero_hashmap: ZeroHashMap<Index32Str, Index32Str> = postcard::from_bytes(&buf).unwrap();

    c.bench_function("zerohashmap/lookup/large", |b| {
        b.iter(|| {
            assert_eq!(
                zero_hashmap.get(black_box(indexify("iu333"))).map(|x| &x.0),
                Some("Inuktitut")
            );
            assert_eq!(
                zero_hashmap.get(black_box(indexify("zz"))).map(|x| &x.0),
                None
            );
        });
    });
}

fn large_zerohashmap_postcard_bytes() -> Vec<u8> {
    postcard::to_stdvec(&build_zerohashmap(true)).unwrap()
}

criterion_group!(benches, overview_bench);
criterion_main!(benches);

/// This type lets us use a u32-index-format VarZeroVec with the ZeroMap.
///
/// Eventually we will have a FormatSelector type that lets us do `ZeroMap<FormatSelector<K, Index32>, V>`
/// (https://github.com/unicode-org/icu4x/issues/2312)
///
/// ,  isn't actually important; it's just more convenient to use make_varule to get the
/// full suite of traits instead of `#[derive(VarULE)]`. (With `#[derive(VarULE)]` we would have to manually
/// define a Serialize implementation, and that would be gnarly)
/// https://github.com/unicode-org/icu4x/issues/2310 tracks being able to do this with derive(ULE)
#[zerovec::make_varule(Index32Str)]
#[zerovec::skip_derive(ZeroMapKV)]
#[derive(Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
#[zerovec::derive(Serialize, Deserialize, Hash)]
pub(crate) struct Index32StrBorrowed<'a>(#[serde(borrow)] pub &'a str);

impl<'a> ZeroMapKV<'a> for Index32Str {
    type Container = VarZeroVec<'a, Index32Str, Index32>;
    type Slice = VarZeroSlice<Index32Str, Index32>;
    type GetType = Index32Str;
    type OwnedType = Box<Index32Str>;
}

#[inline]
fn indexify(s: &str) -> &Index32Str {
    unsafe { &*(s as *const str as *const Index32Str) }
}
