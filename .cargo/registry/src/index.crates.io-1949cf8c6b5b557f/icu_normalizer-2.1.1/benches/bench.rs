// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use criterion::{criterion_group, criterion_main};

mod canonical_composition;
mod canonical_decomposition;
mod composing_normalizer_nfc;
mod composing_normalizer_nfkc;
mod decomposing_normalizer_nfd;
mod decomposing_normalizer_nfkd;
mod utf16_throughput;

criterion_group!(
    benches,
    canonical_composition::criterion_benchmark,
    canonical_decomposition::criterion_benchmark,
    composing_normalizer_nfc::criterion_benchmark,
    composing_normalizer_nfkc::criterion_benchmark,
    decomposing_normalizer_nfd::criterion_benchmark,
    decomposing_normalizer_nfkd::criterion_benchmark,
    utf16_throughput::criterion_benchmark,
);

criterion_main!(benches);
