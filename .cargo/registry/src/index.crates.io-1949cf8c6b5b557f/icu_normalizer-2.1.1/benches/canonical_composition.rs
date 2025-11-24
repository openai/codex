// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, BenchmarkId, Criterion};
use detone::IterDecomposeVietnamese;

use icu_normalizer::properties::{
    CanonicalCompositionBorrowed, CanonicalDecompositionBorrowed, Decomposed,
};
use icu_normalizer::ComposingNormalizerBorrowed;

struct BenchDataContent {
    pub file_name: String,
    pub pairs: Vec<(char, char)>,
}

fn strip_headers(content: &str) -> String {
    content
        .lines()
        .filter(|&s| !s.starts_with('#'))
        .map(|s| s.to_owned())
        .collect::<Vec<String>>()
        .join("\n")
}

fn normalizer_bench_data() -> [BenchDataContent; 16] {
    let nfc_normalizer = ComposingNormalizerBorrowed::new_nfc();

    [
        BenchDataContent {
            file_name: "TestNames_Latin".to_owned(),
            pairs: decompose_data(
                &nfc_normalizer
                    .normalize(&strip_headers(include_str!("./data/TestNames_Latin.txt"))),
            ),
        },
        BenchDataContent {
            file_name: "TestNames_Japanese_h".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestNames_Japanese_h.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestNames_Japanese_k".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestNames_Japanese_k.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestNames_Korean".to_owned(),
            pairs: decompose_data(
                &nfc_normalizer
                    .normalize(&strip_headers(include_str!("./data/TestNames_Korean.txt"))),
            ),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_ar".to_owned(),
            #[cfg(debug_assertions)]
            pairs: Vec::new(),
            #[cfg(not(debug_assertions))]
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_ar.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_de".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_de.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_el".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_el.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_es".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_es.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_fr".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_fr.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_he".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_he.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_pl".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_pl.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_ru".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_ru.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_th".to_owned(),
            #[cfg(debug_assertions)]
            pairs: Vec::new(),
            #[cfg(not(debug_assertions))]
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_th.txt"
            )))),
        },
        BenchDataContent {
            file_name: "TestRandomWordsUDHR_tr".to_owned(),
            pairs: decompose_data(&nfc_normalizer.normalize(&strip_headers(include_str!(
                "./data/TestRandomWordsUDHR_tr.txt"
            )))),
        },
        BenchDataContent {
            file_name: "udhr_vie".to_owned(),
            pairs: decompose_data(
                &nfc_normalizer.normalize(&strip_headers(include_str!("data/wotw.txt"))),
            ),
        },
        BenchDataContent {
            file_name: "udhr_vie_detone".to_owned(),
            pairs: {
                let result: Vec<(char, char)> = nfc_normalizer
                    .normalize(&strip_headers(include_str!("data/wotw.txt")))
                    .chars()
                    .filter_map(|c| {
                        let mut iter = std::iter::once(c).decompose_vietnamese_tones(true);
                        if let Some(base) = iter.next() {
                            iter.next().map(|tone| (base, tone))
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(!result.is_empty());
                result
            },
        },
    ]
}

fn function_under_bench(
    canonical_composer: &CanonicalCompositionBorrowed,
    composable_points: &[(char, char)],
) {
    for pair in composable_points.iter() {
        canonical_composer.compose(pair.0, pair.1);
    }
}

pub fn criterion_benchmark(criterion: &mut Criterion) {
    let group_name = "canonical_composition";
    let mut group = criterion.benchmark_group(group_name);

    let composer = CanonicalCompositionBorrowed::new();

    for bench_data_content in black_box(normalizer_bench_data()) {
        group.bench_function(
            BenchmarkId::from_parameter(format!("from_nfc_{}", bench_data_content.file_name)),
            |bencher| bencher.iter(|| function_under_bench(&composer, &bench_data_content.pairs)),
        );
    }

    group.finish();
}

fn decompose_data(nfc: &str) -> Vec<(char, char)> {
    let decomposer = CanonicalDecompositionBorrowed::new();
    nfc.chars()
        .map(|c| decomposer.decompose(c))
        .filter_map(|decomposed| {
            if let Decomposed::Expansion(a, b) = decomposed {
                Some((a, b))
            } else {
                None
            }
        })
        .collect()
}
