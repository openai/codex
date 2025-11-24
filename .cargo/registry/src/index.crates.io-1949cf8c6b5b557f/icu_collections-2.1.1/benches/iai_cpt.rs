// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use icu_collections::codepointtrie::CodePointTrie;

#[path = "tries/mod.rs"]
mod tries;

// The string has 41 chars.
const SAMPLE_STRING_LATIN1: &str = "Declaration loremips umdolo loremipsompi";
const SAMPLE_STRING_MIXED: &str = "Dèclaråcion ЗАГАЛЬНА 世界人权宣言 𑄟𑄚𑄬𑄭𑄃𑄇𑄴𑄇𑄥𑄧𑄁𑄢𑄴";

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

fn bench_iai_cpt_overview(fast: bool, mixed: bool) {
    // Tests the instructions required to get CPT for 100,000 chars.

    let cpt = if fast {
        get_trie_fast()
    } else {
        get_trie_small()
    };
    let sample = if mixed {
        SAMPLE_STRING_MIXED
    } else {
        SAMPLE_STRING_LATIN1
    };

    let mut i: u8 = 0;
    for c in sample.chars() {
        i = i.wrapping_add(cpt.get32(c as u32))
        //i = i.wrapping_add(1);
    }

    // Ensure the loop is not DCEd
    assert!(i < 255);
}

fn bench_iai_cpt_latin_fast() {
    bench_iai_cpt_overview(true, false);
}

fn bench_iai_cpt_latin_small() {
    bench_iai_cpt_overview(false, false);
}

fn bench_iai_cpt_mixed_fast() {
    bench_iai_cpt_overview(true, true);
}

fn bench_iai_cpt_mixed_small() {
    bench_iai_cpt_overview(false, true);
}

iai::main!(
    bench_iai_cpt_latin_fast,
    bench_iai_cpt_latin_small,
    bench_iai_cpt_mixed_fast,
    bench_iai_cpt_mixed_small,
);
