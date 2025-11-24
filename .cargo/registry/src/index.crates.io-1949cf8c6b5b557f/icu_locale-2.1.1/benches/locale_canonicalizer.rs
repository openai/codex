// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use icu_locale::LocaleCanonicalizer;
use icu_locale::LocaleExpander;
use icu_locale_core::Locale;

fn canonicalize_bench(c: &mut Criterion) {
    let lc = LocaleCanonicalizer::new_common();

    let mut group = c.benchmark_group("uncanonicalized");

    let data: Vec<String> =
        serde_json::from_str(include_str!("fixtures/uncanonicalized-locales.json"))
            .expect("Failed to read a fixture");
    let locales: Vec<Locale> = data.iter().map(|s| s.parse().unwrap()).collect();

    group.bench_function("clone", |b| {
        b.iter(|| {
            for locale in &locales {
                let _ = black_box(locale).clone();
            }
        })
    });

    group.bench_function("canonicalize", |b| {
        b.iter(|| {
            for locale in &locales {
                let mut locale = black_box(locale).clone();
                lc.canonicalize(&mut locale);
            }
        })
    });

    group.finish();
}

fn canonicalize_noop_bench(c: &mut Criterion) {
    let lc = LocaleCanonicalizer::new_common();

    let mut group = c.benchmark_group("canonicalized");

    // None of these locales require canonicalization, so this measures the cost of calling
    // the canonicalizer on locales that will not be modified.
    let data: Vec<String> = serde_json::from_str(include_str!("fixtures/locales.json"))
        .expect("Failed to read a fixture");
    let locales: Vec<Locale> = data.iter().map(|s| s.parse().unwrap()).collect();

    group.bench_function("clone", |b| {
        b.iter(|| {
            for locale in &locales {
                let _ = black_box(locale).clone();
            }
        })
    });

    group.bench_function("canonicalize", |b| {
        b.iter(|| {
            for locale in &locales {
                let mut locale = black_box(locale).clone();
                lc.canonicalize(&mut locale);
            }
        })
    });

    group.finish();
}

fn maximize_bench(c: &mut Criterion) {
    let lc = LocaleExpander::new_common();

    let mut group = c.benchmark_group("likelysubtags");

    let data: Vec<String> = serde_json::from_str(include_str!("fixtures/locales.json"))
        .expect("Failed to read a fixture");
    let locales: Vec<Locale> = data.iter().map(|s| s.parse().unwrap()).collect();

    group.bench_function("maximize", |b| {
        b.iter(|| {
            for locale in &locales {
                let mut locale = locale.clone();
                lc.maximize(black_box(&mut locale.id));
            }
        })
    });

    group.bench_function("minimize", |b| {
        b.iter(|| {
            for locale in &locales {
                let mut locale = locale.clone();
                lc.minimize(black_box(&mut locale.id));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    canonicalize_bench,
    canonicalize_noop_bench,
    maximize_bench
);
criterion_main!(benches);
