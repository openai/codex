// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use icu_collections::codepointtrie::CodePointTrie;

#[path = "tries/mod.rs"]
mod tries;

mod sample_str_lng {
    // "eng" is all ASCII
    pub const ENG: &str = "Universal Declaration of Human Rights";
    // "pcd" has ASCII mixed with low-BMP code points
    pub const PCD: &str = "Dèclaråcion dès dreûts d' l'ome po tos lès payîs dè monde";
    // "ukr" uses low-BMP code points (less than U+1000)
    pub const UKR: &str = "ЗАГАЛЬНА ДЕКЛАРАЦІЯ ПРАВ ЛЮДИНІ";
    // "yue" uses high-BMP code points (greater than U+1000), so it benefits from fast mode
    pub const YUE: &str = "世界人权宣言";
    // "ccp" exercises supplementary code points
    pub const CCP: &str = "𑄟𑄚𑄬𑄭𑄃𑄇𑄴𑄇𑄥𑄧𑄁𑄢𑄴 𑄝𑄬𑄇𑄴𑄅𑄚𑄮𑄢𑄴 𑄟𑄧𑄚𑄳𑄢𑄧𑄧𑄇𑄉𑄮𑄌𑄴";
}

const SAMPLE_STRING_MIXED: &str = "Dèclaråcion ЗАГАЛЬНА 世界人权宣言 𑄟𑄚𑄬𑄭𑄃𑄇𑄴𑄇𑄥𑄧𑄁𑄢𑄴";

/// A function that returns 100 code points in the desired language
fn one_hundred_code_points(sample_str: &str) -> String {
    sample_str.chars().cycle().take(100).collect()
}

fn get_trie_small() -> CodePointTrie<'static, u8> {
    CodePointTrie::try_new(
        tries::gc_small::HEADER,
        tries::gc_small::INDEX,
        tries::gc_small::DATA,
    )
    .unwrap()
}

fn get_trie_fast() -> CodePointTrie<'static, u8> {
    CodePointTrie::try_new(
        tries::gc_fast::HEADER,
        tries::gc_fast::INDEX,
        tries::gc_fast::DATA,
    )
    .unwrap()
}

fn overview_bench(c: &mut Criterion) {
    let s = one_hundred_code_points(SAMPLE_STRING_MIXED);
    let cpt_small = get_trie_small();

    c.bench_function("cpt/overview", |b| {
        b.iter(|| {
            black_box(&s)
                .chars()
                .map(|c| black_box(&cpt_small).get32(c as u32))
                .reduce(|a, b| a.wrapping_add(b))
        });
    });

    c.bench_function("cpt/get_range", |b| {
        b.iter(|| {
            black_box(&s)
                .chars()
                .map(|c| black_box(&cpt_small).get_range(c as u32).unwrap())
                .fold(0u32, |acc, ele| {
                    acc.wrapping_add(ele.range.end() - ele.range.start() + ele.value as u32)
                })
        });
    });

    {
        let cpt_fast = get_trie_fast();
        lang_bench(c, &cpt_small, "small/eng", sample_str_lng::ENG);
        lang_bench(c, &cpt_small, "small/pcd", sample_str_lng::PCD);
        lang_bench(c, &cpt_small, "small/ukr", sample_str_lng::UKR);
        lang_bench(c, &cpt_small, "small/yue", sample_str_lng::YUE);
        lang_bench(c, &cpt_small, "small/ccp", sample_str_lng::CCP);
        lang_bench(c, &cpt_fast, "fast/eng", sample_str_lng::ENG);
        lang_bench(c, &cpt_fast, "fast/pcd", sample_str_lng::PCD);
        lang_bench(c, &cpt_fast, "fast/ukr", sample_str_lng::UKR);
        lang_bench(c, &cpt_fast, "fast/yue", sample_str_lng::YUE);
        lang_bench(c, &cpt_fast, "fast/ccp", sample_str_lng::CCP);
    }
}

fn lang_bench(c: &mut Criterion, cpt: &CodePointTrie<u8>, lid: &str, sample_str: &str) {
    let bench_name = format!("cpt/get/{lid}");
    let s = one_hundred_code_points(sample_str);

    c.bench_function(&bench_name, |b| {
        b.iter(|| {
            black_box(&s)
                .chars()
                .map(|c| black_box(&cpt).get32(c as u32))
                .reduce(|a, b| a.wrapping_add(b))
        });
    });
}

criterion_group!(benches, overview_bench,);
criterion_main!(benches);
