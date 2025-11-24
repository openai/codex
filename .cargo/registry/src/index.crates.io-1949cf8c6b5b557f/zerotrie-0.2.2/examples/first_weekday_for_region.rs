// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// This example demonstrates the use of ZeroTrieSimpleAscii to look up data based on a region code.

#![allow(dead_code)]
#![no_main] // https://github.com/unicode-org/icu4x/issues/395
icu_benchmark_macros::instrument!();

use zerotrie::ZeroTriePerfectHash;
use zerotrie::ZeroTrieSimpleAscii;

mod weekday {
    pub const MON: usize = 1;
    pub const FRI: usize = 5;
    pub const SAT: usize = 6;
    pub const SUN: usize = 7;
}

// This data originated from CLDR 41.
static DATA: &[(&str, usize)] = &[
    ("001", weekday::MON),
    ("AD", weekday::MON),
    ("AE", weekday::SAT),
    ("AF", weekday::SAT),
    ("AG", weekday::SUN),
    ("AI", weekday::MON),
    ("AL", weekday::MON),
    ("AM", weekday::MON),
    ("AN", weekday::MON),
    ("AR", weekday::MON),
    ("AS", weekday::SUN),
    ("AT", weekday::MON),
    ("AU", weekday::MON),
    ("AX", weekday::MON),
    ("AZ", weekday::MON),
    ("BA", weekday::MON),
    ("BD", weekday::SUN),
    ("BE", weekday::MON),
    ("BG", weekday::MON),
    ("BH", weekday::SAT),
    ("BM", weekday::MON),
    ("BN", weekday::MON),
    ("BR", weekday::SUN),
    ("BS", weekday::SUN),
    ("BT", weekday::SUN),
    ("BW", weekday::SUN),
    ("BY", weekday::MON),
    ("BZ", weekday::SUN),
    ("CA", weekday::SUN),
    ("CH", weekday::MON),
    ("CL", weekday::MON),
    ("CM", weekday::MON),
    ("CN", weekday::SUN),
    ("CO", weekday::SUN),
    ("CR", weekday::MON),
    ("CY", weekday::MON),
    ("CZ", weekday::MON),
    ("DE", weekday::MON),
    ("DJ", weekday::SAT),
    ("DK", weekday::MON),
    ("DM", weekday::SUN),
    ("DO", weekday::SUN),
    ("DZ", weekday::SAT),
    ("EC", weekday::MON),
    ("EE", weekday::MON),
    ("EG", weekday::SAT),
    ("ES", weekday::MON),
    ("ET", weekday::SUN),
    ("FI", weekday::MON),
    ("FJ", weekday::MON),
    ("FO", weekday::MON),
    ("FR", weekday::MON),
    ("GB", weekday::MON),
    ("GB-alt-variant", weekday::SUN),
    ("GE", weekday::MON),
    ("GF", weekday::MON),
    ("GP", weekday::MON),
    ("GR", weekday::MON),
    ("GT", weekday::SUN),
    ("GU", weekday::SUN),
    ("HK", weekday::SUN),
    ("HN", weekday::SUN),
    ("HR", weekday::MON),
    ("HU", weekday::MON),
    ("ID", weekday::SUN),
    ("IE", weekday::MON),
    ("IL", weekday::SUN),
    ("IN", weekday::SUN),
    ("IQ", weekday::SAT),
    ("IR", weekday::SAT),
    ("IS", weekday::MON),
    ("IT", weekday::MON),
    ("JM", weekday::SUN),
    ("JO", weekday::SAT),
    ("JP", weekday::SUN),
    ("KE", weekday::SUN),
    ("KG", weekday::MON),
    ("KH", weekday::SUN),
    ("KR", weekday::SUN),
    ("KW", weekday::SAT),
    ("KZ", weekday::MON),
    ("LA", weekday::SUN),
    ("LB", weekday::MON),
    ("LI", weekday::MON),
    ("LK", weekday::MON),
    ("LT", weekday::MON),
    ("LU", weekday::MON),
    ("LV", weekday::MON),
    ("LY", weekday::SAT),
    ("MC", weekday::MON),
    ("MD", weekday::MON),
    ("ME", weekday::MON),
    ("MH", weekday::SUN),
    ("MK", weekday::MON),
    ("MM", weekday::SUN),
    ("MN", weekday::MON),
    ("MO", weekday::SUN),
    ("MQ", weekday::MON),
    ("MT", weekday::SUN),
    ("MV", weekday::FRI),
    ("MX", weekday::SUN),
    ("MY", weekday::MON),
    ("MZ", weekday::SUN),
    ("NI", weekday::SUN),
    ("NL", weekday::MON),
    ("NO", weekday::MON),
    ("NP", weekday::SUN),
    ("NZ", weekday::MON),
    ("OM", weekday::SAT),
    ("PA", weekday::SUN),
    ("PE", weekday::SUN),
    ("PH", weekday::SUN),
    ("PK", weekday::SUN),
    ("PL", weekday::MON),
    ("PR", weekday::SUN),
    ("PT", weekday::SUN),
    ("PY", weekday::SUN),
    ("QA", weekday::SAT),
    ("RE", weekday::MON),
    ("RO", weekday::MON),
    ("RS", weekday::MON),
    ("RU", weekday::MON),
    ("SA", weekday::SUN),
    ("SD", weekday::SAT),
    ("SE", weekday::MON),
    ("SG", weekday::SUN),
    ("SI", weekday::MON),
    ("SK", weekday::MON),
    ("SM", weekday::MON),
    ("SV", weekday::SUN),
    ("SY", weekday::SAT),
    ("TH", weekday::SUN),
    ("TJ", weekday::MON),
    ("TM", weekday::MON),
    ("TR", weekday::MON),
    ("TT", weekday::SUN),
    ("TW", weekday::SUN),
    ("UA", weekday::MON),
    ("UM", weekday::SUN),
    ("US", weekday::SUN),
    ("UY", weekday::MON),
    ("UZ", weekday::MON),
    ("VA", weekday::MON),
    ("VE", weekday::SUN),
    ("VI", weekday::SUN),
    ("VN", weekday::MON),
    ("WS", weekday::SUN),
    ("XK", weekday::MON),
    ("YE", weekday::SUN),
    ("ZA", weekday::SUN),
    ("ZW", weekday::SUN),
];
static TRIE: ZeroTrieSimpleAscii<[u8; 539]> = ZeroTrieSimpleAscii::from_sorted_str_tuples(DATA);

static TRIE_PHF: ZeroTriePerfectHash<[u8; 567]> = ZeroTriePerfectHash::from_store([
    225, 123, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 0, 15, 0,
    81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 79, 65, 66, 67, 68, 69, 70, 71, 72, 73, 75, 74, 48, 76,
    78, 77, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2,
    14, 41, 59, 74, 86, 88, 90, 92, 98, 100, 142, 181, 208, 226, 241, 253, 31, 43, 67, 85, 94, 97,
    121, 136, 178, 65, 134, 196, 69, 79, 83, 85, 1, 2, 3, 129, 129, 129, 129, 201, 65, 68, 69, 71,
    73, 75, 77, 86, 89, 1, 2, 3, 4, 5, 6, 7, 8, 135, 134, 129, 135, 129, 129, 129, 135, 134, 198,
    72, 74, 77, 82, 84, 87, 1, 2, 3, 4, 5, 135, 129, 129, 129, 135, 135, 197, 65, 77, 83, 89, 90,
    1, 2, 3, 4, 129, 135, 135, 129, 129, 196, 65, 69, 73, 78, 1, 2, 3, 129, 135, 135, 129, 83, 135,
    75, 129, 69, 135, 194, 65, 87, 1, 135, 135, 77, 134, 206, 68, 69, 70, 71, 73, 76, 77, 78, 82,
    83, 84, 85, 88, 90, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 129, 134, 134, 135, 129, 129,
    129, 129, 129, 135, 129, 129, 129, 129, 205, 65, 68, 69, 71, 72, 77, 78, 82, 83, 84, 87, 89,
    90, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 129, 135, 129, 129, 134, 129, 129, 135, 135, 135,
    135, 129, 135, 201, 65, 72, 76, 77, 78, 79, 82, 89, 90, 1, 2, 3, 4, 5, 6, 7, 8, 135, 129, 129,
    129, 135, 135, 129, 129, 129, 198, 69, 74, 75, 77, 79, 90, 1, 2, 3, 4, 5, 129, 134, 129, 135,
    135, 134, 197, 67, 69, 71, 83, 84, 1, 2, 3, 4, 129, 129, 134, 129, 135, 196, 73, 74, 79, 82, 1,
    2, 3, 129, 129, 129, 129, 199, 66, 69, 70, 80, 82, 84, 85, 14, 15, 16, 17, 18, 19, 129, 45, 97,
    108, 116, 45, 118, 97, 114, 105, 97, 110, 116, 135, 129, 129, 129, 129, 135, 135, 196, 75, 78,
    82, 85, 1, 2, 3, 135, 135, 129, 129, 200, 68, 69, 76, 78, 81, 82, 83, 84, 1, 2, 3, 4, 5, 6, 7,
    135, 129, 135, 135, 134, 134, 129, 129, 198, 69, 71, 72, 82, 87, 90, 1, 2, 3, 4, 5, 135, 129,
    135, 135, 134, 129, 195, 77, 79, 80, 1, 2, 135, 134, 135, 48, 49, 129, 200, 65, 66, 73, 75, 84,
    85, 86, 89, 1, 2, 3, 4, 5, 6, 7, 135, 129, 129, 129, 129, 129, 129, 134, 197, 73, 76, 79, 80,
    90, 1, 2, 3, 4, 135, 129, 129, 135, 129, 206, 67, 68, 69, 72, 75, 77, 78, 79, 81, 84, 86, 88,
    89, 90, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 129, 129, 129, 135, 129, 135, 129, 135, 129,
    135, 133, 135, 129, 135, 200, 65, 69, 72, 75, 76, 82, 84, 89, 1, 2, 3, 4, 5, 6, 7, 135, 135,
    135, 135, 129, 135, 135, 135,
]);

fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}

fn main() {
    // Un-comment to re-generate the bytes (printed to the terminal)
    // let trie_phf = DATA.iter().copied().collect::<ZeroTriePerfectHash<Vec<_>>>();
    // assert_eq!(trie_phf.as_bytes(), TRIE_PHF.as_bytes());

    assert_eq!(black_box(TRIE_PHF).get(b"MV"), Some(weekday::FRI));
}
