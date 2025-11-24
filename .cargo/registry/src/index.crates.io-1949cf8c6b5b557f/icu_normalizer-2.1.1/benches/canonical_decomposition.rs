// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, BenchmarkId, Criterion};

use icu_normalizer::properties::CanonicalDecompositionBorrowed;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};

struct BenchDataContent {
    pub file_name: String,
    pub nfc: String,
    pub nfd: String,
    pub nfkc: String,
    pub nfkd: String,
}

fn strip_headers(content: &str) -> String {
    content
        .lines()
        .filter(|&s| !s.starts_with('#'))
        .map(|s| s.to_owned())
        .collect::<Vec<String>>()
        .join("\n")
}

fn normalizer_bench_data() -> [BenchDataContent; 15] {
    let nfc_normalizer = ComposingNormalizerBorrowed::new_nfc();
    let nfd_normalizer = DecomposingNormalizerBorrowed::new_nfd();
    let nfkc_normalizer = ComposingNormalizerBorrowed::new_nfkc();
    let nfkd_normalizer = DecomposingNormalizerBorrowed::new_nfkd();

    let content_latin: (&str, &str) = (
        "TestNames_Latin",
        &strip_headers(include_str!("./data/TestNames_Latin.txt")),
    );
    let content_jp_h: (&str, &str) = (
        "TestNames_Japanese_h",
        &strip_headers(include_str!("./data/TestNames_Japanese_h.txt")),
    );
    let content_jp_k: (&str, &str) = (
        "TestNames_Japanese_k",
        &strip_headers(include_str!("./data/TestNames_Japanese_k.txt")),
    );
    let content_korean: (&str, &str) = (
        "TestNames_Korean",
        &strip_headers(include_str!("./data/TestNames_Korean.txt")),
    );
    let content_random_words_ar: (&str, &str) = (
        "TestRandomWordsUDHR_ar",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_ar.txt")),
    );
    let content_random_words_de: (&str, &str) = (
        "TestRandomWordsUDHR_de",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_de.txt")),
    );
    let content_random_words_el: (&str, &str) = (
        "TestRandomWordsUDHR_el",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_el.txt")),
    );
    let content_random_words_es: (&str, &str) = (
        "TestRandomWordsUDHR_es",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_es.txt")),
    );
    let content_random_words_fr: (&str, &str) = (
        "TestRandomWordsUDHR_fr",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_fr.txt")),
    );
    let content_random_words_he: (&str, &str) = (
        "TestRandomWordsUDHR_he",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_he.txt")),
    );
    let content_random_words_pl: (&str, &str) = (
        "TestRandomWordsUDHR_pl",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_pl.txt")),
    );
    let content_random_words_ru: (&str, &str) = (
        "TestRandomWordsUDHR_ru",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_ru.txt")),
    );
    let content_random_words_th: (&str, &str) = (
        "TestRandomWordsUDHR_th",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_th.txt")),
    );
    let content_random_words_tr: (&str, &str) = (
        "TestRandomWordsUDHR_tr",
        &strip_headers(include_str!("./data/TestRandomWordsUDHR_tr.txt")),
    );
    let content_viet: (&str, &str) = ("udhr_vie", &strip_headers(include_str!("data/wotw.txt")));

    [
        content_latin,
        content_viet,
        content_jp_k,
        content_jp_h,
        content_korean,
        content_random_words_ru,
        content_random_words_ar,
        content_random_words_el,
        content_random_words_es,
        content_random_words_fr,
        content_random_words_tr,
        content_random_words_th,
        content_random_words_pl,
        content_random_words_he,
        content_random_words_de,
    ]
    .map(|(file_name, raw_content)| BenchDataContent {
        file_name: file_name.to_owned(),
        nfc: nfc_normalizer.normalize(raw_content).to_string(),
        nfd: nfd_normalizer.normalize(raw_content).to_string(),
        nfkc: nfkc_normalizer.normalize(raw_content).to_string(),
        nfkd: nfkd_normalizer.normalize(raw_content).to_string(),
    })
}

#[cfg(debug_assertions)]
fn function_under_bench(
    _canonical_decomposer: &CanonicalDecompositionBorrowed,
    _decomposable_points: &str,
) {
    // using debug assertion fails some test.
    // "cargo test --bench bench" will pass
    // "cargo bench" will work as expected, because the profile doesn't include debug assertions.
}

#[cfg(not(debug_assertions))]
fn function_under_bench(
    canonical_decomposer: &CanonicalDecompositionBorrowed,
    decomposable_points: &str,
) {
    decomposable_points.chars().for_each(|point| {
        canonical_decomposer.decompose(point);
    });
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let group_name = "canonical_decomposition";
    let mut group = criterion.benchmark_group(group_name);

    let decomposer = CanonicalDecompositionBorrowed::new();

    for bench_data_content in black_box(normalizer_bench_data()) {
        group.bench_function(
            BenchmarkId::from_parameter(format!("from_nfc_{}", bench_data_content.file_name)),
            |bencher| bencher.iter(|| function_under_bench(&decomposer, &bench_data_content.nfc)),
        );
        group.bench_function(
            BenchmarkId::from_parameter(format!("from_nfd_{}", bench_data_content.file_name)),
            |bencher| bencher.iter(|| function_under_bench(&decomposer, &bench_data_content.nfd)),
        );
        group.bench_function(
            BenchmarkId::from_parameter(format!("from_nfkc_{}", bench_data_content.file_name)),
            |bencher| bencher.iter(|| function_under_bench(&decomposer, &bench_data_content.nfkc)),
        );
        group.bench_function(
            BenchmarkId::from_parameter(format!("from_nfkd_{}", bench_data_content.file_name)),
            |bencher| bencher.iter(|| function_under_bench(&decomposer, &bench_data_content.nfkd)),
        );
    }
    group.finish();
}
