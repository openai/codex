use criterion::{criterion_group, criterion_main, Criterion};
use rustc_hex::{FromHex, ToHex};

const DATA: &[u8] = include_bytes!("../src/lib.rs");

fn bench_encode(c: &mut Criterion) {
    c.bench_function("hex_encode", |b| b.iter(|| hex::encode(DATA)));

    c.bench_function("rustc_hex_encode", |b| b.iter(|| DATA.to_hex::<String>()));

    c.bench_function("faster_hex_encode", |b| {
        b.iter(|| faster_hex::hex_string(DATA).unwrap())
    });

    c.bench_function("faster_hex_encode_fallback", |b| {
        b.iter(|| {
            let mut dst = vec![0; DATA.len() * 2];
            faster_hex::hex_encode_fallback(DATA, &mut dst);
            dst
        })
    });
}

fn bench_decode(c: &mut Criterion) {
    c.bench_function("hex_decode", |b| {
        let hex = hex::encode(DATA);
        b.iter(|| hex::decode(&hex).unwrap())
    });

    c.bench_function("rustc_hex_decode", |b| {
        let hex = DATA.to_hex::<String>();
        b.iter(|| hex.from_hex::<Vec<u8>>().unwrap())
    });

    c.bench_function("faster_hex_decode", move |b| {
        let hex = faster_hex::hex_string(DATA).unwrap();
        let len = DATA.len();
        let mut dst = vec![0; len];

        b.iter(|| faster_hex::hex_decode(hex.as_bytes(), &mut dst).unwrap())
    });

    c.bench_function("faster_hex_decode_unchecked", |b| {
        let hex = faster_hex::hex_string(DATA).unwrap();
        let len = DATA.len();
        let mut dst = vec![0; len];

        b.iter(|| faster_hex::hex_decode_unchecked(hex.as_bytes(), &mut dst))
    });

    c.bench_function("faster_hex_decode_fallback", |b| {
        let hex = faster_hex::hex_string(DATA).unwrap();
        let len = DATA.len();
        let mut dst = vec![0; len];

        b.iter(|| faster_hex::hex_decode_fallback(hex.as_bytes(), &mut dst))
    });
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
