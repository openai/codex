use criterion::{black_box, criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use half::prelude::*;
use std::{f32, f64, iter};

const SIMD_LARGE_BENCH_SLICE_LEN: usize = 1024;

fn bench_f32_to_f16(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert f16 From f32");
    for val in &[
        0.,
        -0.,
        1.,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NAN,
        f32::consts::E,
        f32::consts::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("f16::from_f32", val), val, |b, i| {
            b.iter(|| f16::from_f32(*i))
        });
    }
}

fn bench_f64_to_f16(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert f16 From f64");
    for val in &[
        0.,
        -0.,
        1.,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
        f64::consts::E,
        f64::consts::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("f16::from_f64", val), val, |b, i| {
            b.iter(|| f16::from_f64(*i))
        });
    }
}

fn bench_f16_to_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert f16 to f32");
    for val in &[
        f16::ZERO,
        f16::NEG_ZERO,
        f16::ONE,
        f16::MIN,
        f16::MAX,
        f16::MIN_POSITIVE,
        f16::NEG_INFINITY,
        f16::INFINITY,
        f16::NAN,
        f16::E,
        f16::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("f16::to_f32", val), val, |b, i| {
            b.iter(|| i.to_f32())
        });
    }
}

fn bench_f16_to_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert f16 to f64");
    for val in &[
        f16::ZERO,
        f16::NEG_ZERO,
        f16::ONE,
        f16::MIN,
        f16::MAX,
        f16::MIN_POSITIVE,
        f16::NEG_INFINITY,
        f16::INFINITY,
        f16::NAN,
        f16::E,
        f16::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("f16::to_f64", val), val, |b, i| {
            b.iter(|| i.to_f64())
        });
    }
}

criterion_group!(
    f16_sisd,
    bench_f32_to_f16,
    bench_f64_to_f16,
    bench_f16_to_f32,
    bench_f16_to_f64
);

fn bench_slice_f32_to_f16(c: &mut Criterion) {
    let mut constant_buffer = [f16::ZERO; 11];
    let constants = [
        0.,
        -0.,
        1.,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NAN,
        f32::consts::E,
        f32::consts::PI,
    ];
    c.bench_function(
        "HalfFloatSliceExt::convert_from_f32_slice/constants",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&mut constant_buffer).convert_from_f32_slice(black_box(&constants)))
        },
    );

    let large: Vec<_> = iter::repeat(0)
        .enumerate()
        .map(|(i, _)| i as f32)
        .take(SIMD_LARGE_BENCH_SLICE_LEN)
        .collect();
    let mut large_buffer = [f16::ZERO; SIMD_LARGE_BENCH_SLICE_LEN];
    c.bench_function(
        "HalfFloatSliceExt::convert_from_f32_slice/large",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&mut large_buffer).convert_from_f32_slice(black_box(&large)))
        },
    );
}

fn bench_slice_f64_to_f16(c: &mut Criterion) {
    let mut constant_buffer = [f16::ZERO; 11];
    let constants = [
        0.,
        -0.,
        1.,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
        f64::consts::E,
        f64::consts::PI,
    ];
    c.bench_function(
        "HalfFloatSliceExt::convert_from_f64_slice/constants",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&mut constant_buffer).convert_from_f64_slice(black_box(&constants)))
        },
    );

    let large: Vec<_> = iter::repeat(0)
        .enumerate()
        .map(|(i, _)| i as f64)
        .take(SIMD_LARGE_BENCH_SLICE_LEN)
        .collect();
    let mut large_buffer = [f16::ZERO; SIMD_LARGE_BENCH_SLICE_LEN];
    c.bench_function(
        "HalfFloatSliceExt::convert_from_f64_slice/large",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&mut large_buffer).convert_from_f64_slice(black_box(&large)))
        },
    );
}

fn bench_slice_f16_to_f32(c: &mut Criterion) {
    let mut constant_buffer = [0f32; 11];
    let constants = [
        f16::ZERO,
        f16::NEG_ZERO,
        f16::ONE,
        f16::MIN,
        f16::MAX,
        f16::MIN_POSITIVE,
        f16::NEG_INFINITY,
        f16::INFINITY,
        f16::NAN,
        f16::E,
        f16::PI,
    ];
    c.bench_function(
        "HalfFloatSliceExt::convert_to_f32_slice/constants",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&constants).convert_to_f32_slice(black_box(&mut constant_buffer)))
        },
    );

    let large: Vec<_> = iter::repeat(0)
        .enumerate()
        .map(|(i, _)| f16::from_f32(i as f32))
        .take(SIMD_LARGE_BENCH_SLICE_LEN)
        .collect();
    let mut large_buffer = [0f32; SIMD_LARGE_BENCH_SLICE_LEN];
    c.bench_function(
        "HalfFloatSliceExt::convert_to_f32_slice/large",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&large).convert_to_f32_slice(black_box(&mut large_buffer)))
        },
    );
}

fn bench_slice_f16_to_f64(c: &mut Criterion) {
    let mut constant_buffer = [0f64; 11];
    let constants = [
        f16::ZERO,
        f16::NEG_ZERO,
        f16::ONE,
        f16::MIN,
        f16::MAX,
        f16::MIN_POSITIVE,
        f16::NEG_INFINITY,
        f16::INFINITY,
        f16::NAN,
        f16::E,
        f16::PI,
    ];
    c.bench_function(
        "HalfFloatSliceExt::convert_to_f64_slice/constants",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&constants).convert_to_f64_slice(black_box(&mut constant_buffer)))
        },
    );

    let large: Vec<_> = iter::repeat(0)
        .enumerate()
        .map(|(i, _)| f16::from_f64(i as f64))
        .take(SIMD_LARGE_BENCH_SLICE_LEN)
        .collect();
    let mut large_buffer = [0f64; SIMD_LARGE_BENCH_SLICE_LEN];
    c.bench_function(
        "HalfFloatSliceExt::convert_to_f64_slice/large",
        |b: &mut Bencher<'_>| {
            b.iter(|| black_box(&large).convert_to_f64_slice(black_box(&mut large_buffer)))
        },
    );
}

criterion_group!(
    f16_simd,
    bench_slice_f32_to_f16,
    bench_slice_f64_to_f16,
    bench_slice_f16_to_f32,
    bench_slice_f16_to_f64
);

fn bench_f32_to_bf16(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert bf16 From f32");
    for val in &[
        0.,
        -0.,
        1.,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NAN,
        f32::consts::E,
        f32::consts::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("bf16::from_f32", val), val, |b, i| {
            b.iter(|| bf16::from_f32(*i))
        });
    }
}

fn bench_f64_to_bf16(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert bf16 From f64");
    for val in &[
        0.,
        -0.,
        1.,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
        f64::consts::E,
        f64::consts::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("bf16::from_f64", val), val, |b, i| {
            b.iter(|| bf16::from_f64(*i))
        });
    }
}

fn bench_bf16_to_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert bf16 to f32");
    for val in &[
        bf16::ZERO,
        bf16::NEG_ZERO,
        bf16::ONE,
        bf16::MIN,
        bf16::MAX,
        bf16::MIN_POSITIVE,
        bf16::NEG_INFINITY,
        bf16::INFINITY,
        bf16::NAN,
        bf16::E,
        bf16::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("bf16::to_f32", val), val, |b, i| {
            b.iter(|| i.to_f32())
        });
    }
}

fn bench_bf16_to_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("Convert bf16 to f64");
    for val in &[
        bf16::ZERO,
        bf16::NEG_ZERO,
        bf16::ONE,
        bf16::MIN,
        bf16::MAX,
        bf16::MIN_POSITIVE,
        bf16::NEG_INFINITY,
        bf16::INFINITY,
        bf16::NAN,
        bf16::E,
        bf16::PI,
    ] {
        group.bench_with_input(BenchmarkId::new("bf16::to_f64", val), val, |b, i| {
            b.iter(|| i.to_f64())
        });
    }
}

criterion_group!(
    bf16_sisd,
    bench_f32_to_bf16,
    bench_f64_to_bf16,
    bench_bf16_to_f32,
    bench_bf16_to_f64
);

criterion_main!(f16_sisd, bf16_sisd, f16_simd);
