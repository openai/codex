// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use rand::SeedableRng;
use rand_distr::{Distribution, Triangular};
use rand_pcg::Lcg64Xsh32;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use icu_decimal::input::Decimal;
use icu_decimal::{DecimalFormatter, DecimalFormatterPreferences};
use icu_locale_core::locale;

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

fn overview_bench(c: &mut Criterion) {
    let nums = triangular_nums(1e9);
    let locale = locale!("en-US");
    let prefs = DecimalFormatterPreferences::from(&locale);
    let options = Default::default();
    c.bench_function("icu_decimal/overview", |b| {
        b.iter(|| {
            // This benchmark demonstrates the performance of the format function on 1000 numbers
            // ranging from -1e9 to 1e9.
            let formatter = DecimalFormatter::try_new(prefs, options).unwrap();
            for &num in &nums {
                let decimal = Decimal::from(black_box(num));
                formatter.format_to_string(&decimal);
            }
        });
    });
}

criterion_group!(benches, overview_bench);
criterion_main!(benches);
