// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::str::FromStr;
use rand::SeedableRng;
use rand_distr::{Distribution, Triangular};
use rand_pcg::Lcg64Xsh32;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use fixed_decimal::Decimal;

// TODO: move to helpers.rs
fn triangular_nums(range: f64) -> Vec<isize> {
    // Use Lcg64Xsh32, a small, fast PRNG.
    // Generate 1000 numbers between -range and +range, weighted around 0.
    let rng = Lcg64Xsh32::seed_from_u64(2020);
    let dist = Triangular::new(-range, range, 0.0).unwrap();
    dist.sample_iter(rng)
        .take(1000)
        .map(|v| v as isize)
        .collect()
}

// TODO: move to helpers.rs
fn triangular_floats(range: f64) -> impl Iterator<Item = f64> {
    // Use Lcg64Xsh32, a small, fast PRNG.s
    // Generate 1000 numbers between -range and +range, weighted around 0.
    let rng = Lcg64Xsh32::seed_from_u64(2024);
    let dist = Triangular::new(-range, range, 0.0).unwrap();
    dist.sample_iter(rng).take(1000)
}

fn overview_bench(c: &mut Criterion) {
    let nums = triangular_nums(1e4);
    let values: Vec<_> = nums.iter().map(|n| n.to_string()).collect();
    c.bench_function("fixed_decimal/overview", |b| {
        #[allow(clippy::suspicious_map)]
        b.iter(|| {
            // This benchmark focuses on short numbers and performs:
            // * Construction of FixedDecimals from isize
            // * Construction of FixedDecimals from strings
            // * Serialization of FixedDecimal to string
            nums.iter()
                .map(|v| black_box(*v))
                .map(Decimal::from)
                .count();
            let fds: Vec<_> = values
                .iter()
                .map(black_box)
                .map(|v| Decimal::from_str(v).expect("Failed to parse"))
                .collect();
            fds.iter().map(black_box).map(|v| v.to_string()).count();
        });
    });

    {
        smaller_isize_benches(c);
        larger_isize_benches(c);
        to_string_benches(c);
        from_string_benches(c);
        rounding_benches(c);
    }
}

fn smaller_isize_benches(c: &mut Criterion) {
    // Smaller nums: -1e4 to 1e4
    let nums = triangular_nums(1e4);

    // Note: this could be bench_function_with_inputs, but there are 1000 random inputs.
    // Instead, consider all inputs together in the same benchmark.
    c.bench_function("isize/smaller", |b| {
        b.iter(|| {
            #[allow(clippy::suspicious_map)]
            nums.iter()
                .map(|v| black_box(*v))
                .map(Decimal::from)
                .count()
        });
    });
}

fn larger_isize_benches(c: &mut Criterion) {
    // Larger nums: -1e16 to 1e16
    let nums = triangular_nums(1e16);

    // Note: this could be bench_function_with_inputs, but there are 1000 random inputs.
    // Instead, consider all inputs together in the same benchmark.
    c.bench_function("isize/larger", |b| {
        b.iter(|| {
            #[allow(clippy::suspicious_map)]
            nums.iter()
                .map(|v| black_box(*v))
                .map(Decimal::from)
                .count()
        });
    });
}

fn to_string_benches(c: &mut Criterion) {
    use criterion::BenchmarkId;
    use writeable::Writeable;

    let objects = [
        {
            let mut fd = Decimal::from(2250);
            fd.multiply_pow10(-2);
            fd
        },
        Decimal::from(908070605040302010u128),
    ];

    {
        let mut group = c.benchmark_group("to_string/to_string");
        for object in objects.iter() {
            group.bench_with_input(
                BenchmarkId::from_parameter(object.to_string()),
                object,
                |b, object| b.iter(|| object.to_string()),
            );
        }
        group.finish();
    }

    {
        let mut group = c.benchmark_group("to_string/write_to");
        for object in objects.iter() {
            group.bench_with_input(
                BenchmarkId::from_parameter(object.to_string()),
                object,
                |b, object| b.iter(|| object.write_to_string().into_owned()),
            );
        }
        group.finish();
    }
}

fn from_string_benches(c: &mut Criterion) {
    use criterion::BenchmarkId;

    let objects = [
        "0012.3400",
        "00.0012216734340",
        "00002342561123400.0",
        "-00123400",
        "922337203685477580898230948203840239384.9823094820384023938423424",
        "0.000000001",
        "1000000001",
        &{
            let mut x = format!("{:0fill$}", 0, fill = i16::MAX as usize + 1);
            x.push('.');
            x.push_str(&format!("{:0fill$}", 0, fill = i16::MAX as usize + 1));
            x
        },
    ];

    {
        let mut group = c.benchmark_group("from_string");
        for object in objects.iter() {
            group.bench_with_input(
                BenchmarkId::from_parameter(object.to_string()),
                object,
                |b, object| b.iter(|| Decimal::from_str(object).unwrap()),
            );
        }
        group.finish();
    }
}

fn rounding_benches(c: &mut Criterion) {
    use fixed_decimal::{FloatPrecision, SignedRoundingMode, UnsignedRoundingMode};
    const ROUNDING_MODES: [(&str, SignedRoundingMode); 9] = [
        ("ceil", SignedRoundingMode::Ceil),
        ("floor", SignedRoundingMode::Floor),
        (
            "expand",
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        ),
        (
            "trunc",
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        ),
        ("half_ceil", SignedRoundingMode::HalfCeil),
        ("half_floor", SignedRoundingMode::HalfFloor),
        (
            "half_expand",
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        ),
        (
            "half_trunc",
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        ),
        (
            "half_even",
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        ),
    ];

    let nums: Vec<_> = triangular_floats(1e7)
        .map(|f| Decimal::try_from_f64(f, FloatPrecision::RoundTrip).unwrap())
        .collect();
    let mut group = c.benchmark_group("rounding");

    for (name, rounding_mode) in ROUNDING_MODES {
        group.bench_function(name, |b| {
            b.iter(|| {
                for offset in -5..=5 {
                    nums.iter()
                        .cloned()
                        .map(|num| {
                            Decimal::rounded_with_mode(black_box(num), offset, rounding_mode)
                        })
                        .for_each(|num| {
                            black_box(num);
                        });
                }
            })
        });
    }

    group.finish()
}

criterion_group!(benches, overview_bench,);
criterion_main!(benches);
