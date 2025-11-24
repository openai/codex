// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

mod fixtures;

use icu_locale::{LocaleCanonicalizer, LocaleExpander, TransformResult};
use icu_locale_core::Locale;
use writeable::assert_writeable_eq;

#[test]
fn test_maximize() {
    let lc = LocaleExpander::new_extended();

    let testcases: Vec<fixtures::CanonicalizationTest> =
        serde_json::from_str(include_str!("fixtures/maximize.json"))
            .expect("Failed to read a fixture");

    for case in testcases {
        if let Some(true) = case.disabled {
            continue;
        }
        let mut locale: Locale = case.input.parse().unwrap();
        let unmodified = locale.clone();
        let result = lc.maximize(&mut locale.id);
        assert_writeable_eq!(locale, case.output, "{:?}", case);
        if result == TransformResult::Modified {
            assert_ne!(locale, unmodified, "{case:?}");
        } else {
            assert_eq!(locale, unmodified, "{case:?}");
        }
    }
}

#[test]
fn test_minimize() {
    let lc = LocaleExpander::new_extended();

    let testcases: Vec<fixtures::CanonicalizationTest> =
        serde_json::from_str(include_str!("fixtures/minimize.json"))
            .expect("Failed to read a fixture");

    for case in testcases {
        if let Some(true) = case.disabled {
            continue;
        }
        let mut locale: Locale = case.input.parse().unwrap();
        let unmodified = locale.clone();
        let result = lc.minimize(&mut locale.id);
        assert_writeable_eq!(locale, case.output, "{:?}", case);
        if result == TransformResult::Modified {
            assert_ne!(locale, unmodified, "{case:?}");
        } else {
            assert_eq!(locale, unmodified, "{case:?}");
        }
    }
}

#[test]
fn test_canonicalize() {
    let lc = LocaleCanonicalizer::new_extended();

    let testcases: Vec<fixtures::CanonicalizationTest> =
        serde_json::from_str(include_str!("fixtures/canonicalize.json"))
            .expect("Failed to read a fixture");

    for case in testcases {
        if let Some(true) = case.disabled {
            continue;
        }
        let mut locale: Locale = case.input.parse().expect("Unable to parse input");
        let unmodified = locale.clone();
        let result = lc.canonicalize(&mut locale);
        assert_writeable_eq!(locale, case.output, "{:?}", case);
        if result == TransformResult::Modified {
            assert_ne!(locale, unmodified, "{case:?}");
        } else {
            assert_eq!(locale, unmodified, "{case:?}");
        }
    }
}
