// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

mod fixtures;
mod helpers;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use icu_locale_core::LanguageIdentifier;

fn langid_benches(c: &mut Criterion) {
    let data = serde_json::from_str::<fixtures::LocaleList>(include_str!("fixtures/langid.json"))
        .expect("Failed to read a fixture");

    // Overview
    {
        let mut group = c.benchmark_group("langid");

        overview!(group, LanguageIdentifier, &data.canonicalized, "en-US");

        group.finish();
    }

    {
        use criterion::BenchmarkId;

        // Construct
        {
            let mut group = c.benchmark_group("langid/construct");

            construct!(group, LanguageIdentifier, "langid", &data.canonicalized);

            group.finish();
        }

        // Stringify
        {
            let mut group = c.benchmark_group("langid/to_string");

            let langids: Vec<LanguageIdentifier> = data
                .canonicalized
                .iter()
                .map(|s| s.parse().unwrap())
                .collect();

            to_string!(group, LanguageIdentifier, "langid", &langids);

            group.finish();
        }

        // Compare
        {
            let mut group = c.benchmark_group("langid/compare");

            let langids: Vec<LanguageIdentifier> = data
                .canonicalized
                .iter()
                .map(|s| s.parse().unwrap())
                .collect();
            let langids2: Vec<LanguageIdentifier> = data
                .canonicalized
                .iter()
                .map(|s| s.parse().unwrap())
                .collect();

            compare_struct!(group, LanguageIdentifier, "langid", &langids, &langids2);

            compare_str!(
                group,
                LanguageIdentifier,
                "langid",
                &langids,
                &data.canonicalized
            );

            group.finish();
        }

        // Canonicalize
        {
            let mut group = c.benchmark_group("langid/canonicalize");

            canonicalize!(group, LanguageIdentifier, "langid", &data.casing);

            group.finish();
        }
    }
}

criterion_group!(benches, langid_benches,);
criterion_main!(benches);
