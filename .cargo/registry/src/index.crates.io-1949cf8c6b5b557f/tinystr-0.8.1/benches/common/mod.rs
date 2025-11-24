// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// This file was adapted from parts of https://github.com/zbraniecki/tinystr

pub static STRINGS_4: &[&str] = &[
    "US", "GB", "AR", "Hans", "CN", "AT", "PL", "FR", "AT", "Cyrl", "SR", "NO", "FR", "MK", "UK",
];

pub static STRINGS_8: &[&str] = &[
    "Latn", "windows", "AR", "Hans", "macos", "AT", "pl", "FR", "en", "Cyrl", "SR", "NO", "419",
    "und", "UK",
];

pub static STRINGS_16: &[&str] = &[
    "Latn",
    "windows",
    "AR",
    "Hans",
    "macos",
    "AT",
    "infiniband",
    "FR",
    "en",
    "Cyrl",
    "FromIntegral",
    "NO",
    "419",
    "MacintoshOSX2019",
    "UK",
];

#[macro_export]
macro_rules! bench_block {
    ($c:expr, $name:expr, $action:ident) => {
        let mut group4 = $c.benchmark_group(&format!("{}/4", $name));
        group4.bench_function("String", $action!(String, STRINGS_4));
        group4.bench_function("TinyAsciiStr<4>", $action!(TinyAsciiStr<4>, STRINGS_4));
        group4.bench_function("TinyAsciiStr<8>", $action!(TinyAsciiStr<8>, STRINGS_4));
        group4.bench_function("TinyAsciiStr<16>", $action!(TinyAsciiStr<16>, STRINGS_4));
        group4.finish();

        let mut group8 = $c.benchmark_group(&format!("{}/8", $name));
        group8.bench_function("String", $action!(String, STRINGS_8));
        group8.bench_function("TinyAsciiStr<8>", $action!(TinyAsciiStr<8>, STRINGS_8));
        group8.bench_function("TinyAsciiStr<16>", $action!(TinyAsciiStr<16>, STRINGS_8));
        group8.finish();

        let mut group16 = $c.benchmark_group(&format!("{}/16", $name));
        group16.bench_function("String", $action!(String, STRINGS_16));
        group16.bench_function("TinyAsciiStr<16>", $action!(TinyAsciiStr<16>, STRINGS_16));
        group16.finish();
    };
}
