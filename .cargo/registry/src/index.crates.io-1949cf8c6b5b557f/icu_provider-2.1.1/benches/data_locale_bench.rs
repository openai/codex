// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

extern crate alloc;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use icu_provider::prelude::*;
use std::str::FromStr;
use writeable::Writeable;

static BCP47_STRINGS: &[&str] = &[
    "ca",
    "ca-ES",
    "ca-ES-u-sd-esar",
    "ca-ES-valencia",
    "ca-ES-xyzabc",
    "cat",
    "pl-Latn-PL",
    "und",
    "und-fonipa",
    "zh",
];

fn overview_bench(c: &mut Criterion) {
    c.bench_function("data_locale/overview", |b| {
        b.iter(|| {
            for s in black_box(BCP47_STRINGS).iter() {
                let loc = DataLocale::from_str(s).unwrap();
                let s = loc.write_to_string();
                loc.strict_cmp(s.as_bytes());
            }
        });
    });

    data_locale_bench(c);
}

fn data_locale_bench(c: &mut Criterion) {
    c.bench_function("data_locale/parse", |b| {
        b.iter(|| {
            for s in black_box(BCP47_STRINGS).iter() {
                DataLocale::from_str(s).unwrap();
            }
        });
    });

    let data_locales: Vec<DataLocale> = BCP47_STRINGS.iter().map(|s| s.parse().unwrap()).collect();

    c.bench_function("data_locale/write_to_string", |b| {
        b.iter(|| {
            for loc in black_box(&data_locales).iter() {
                loc.write_to_string();
            }
        });
    });
    c.bench_function("data_locale/strict_cmp", |b| {
        b.iter(|| {
            for loc in black_box(&data_locales).iter() {
                for s in black_box(BCP47_STRINGS).iter() {
                    loc.strict_cmp(s.as_bytes());
                }
            }
        });
    });
}

criterion_group!(benches, overview_bench,);
criterion_main!(benches);
