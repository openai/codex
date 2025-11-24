// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

mod fixtures;

use std::convert::TryInto;
use writeable::*;

use icu_locale_core::{LanguageIdentifier, Locale, ParseError};

type Result = std::result::Result<Locale, ParseError>;

fn test_langid_fixtures(tests: Vec<fixtures::LocaleTest>) {
    for test in tests {
        match test.output {
            fixtures::LocaleInfo::String(s) => {
                let input: Locale = test.input.try_into().expect("Parsing failed.");
                assert_writeable_eq!(input, s);
            }
            fixtures::LocaleInfo::Error(err) => {
                let err: ParseError = err.into();
                let input: Result = test.input.try_into();
                assert_eq!(input, Err(err));
            }
            fixtures::LocaleInfo::Identifier(ident) => {
                let input: Locale = test.input.try_into().expect("Parsing failed.");
                let output: Locale = ident.clone().try_into().expect("Parsing failed.");
                assert_eq!(input, output);
                assert_writeable_eq!(input, ident.identifier);
            }
            fixtures::LocaleInfo::Object(o) => {
                let input: Locale = test.input.try_into().expect("Parsing failed.");
                let output: Locale = o.try_into().expect("Parsing failed.");
                assert_eq!(input, output);
            }
        }
    }
}

#[test]
fn test_locale_parsing() {
    let data = serde_json::from_str(include_str!("fixtures/locale.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_locale_invalid() {
    let data = serde_json::from_str(include_str!("fixtures/invalid-extensions.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_locale_is_empty() {
    let locale: Locale = Locale::UNKNOWN;
    assert!(locale.extensions.is_empty());
    assert_writeable_eq!(locale, "und");
}

#[test]
fn test_locale_conversions() {
    let locale: Locale = Locale::UNKNOWN;
    let langid: LanguageIdentifier = locale.clone().into();
    let locale2: Locale = langid.into();
    assert_eq!(locale, locale2);
}

#[test]
fn test_locale_canonicalize() {
    let data = serde_json::from_str(include_str!("fixtures/canonicalize.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_locale_normalizing_eq_str() {
    let tests: Vec<fixtures::LocaleTest> =
        serde_json::from_str(include_str!("fixtures/locale.json"))
            .expect("Failed to read a fixture");
    for test in tests {
        let parsed: Locale = test.input.try_into().expect("Parsing failed.");
        assert!(parsed.normalizing_eq(&parsed.write_to_string()));
    }

    // Check that trailing characters are not ignored
    let locale: Locale = "en".parse().expect("Parsing failed.");
    assert!(!locale.normalizing_eq("en-US"));
}

#[test]
fn test_locale_strict_cmp() {
    let tests: Vec<fixtures::LocaleTest> =
        serde_json::from_str(include_str!("fixtures/locale.json"))
            .expect("Failed to read a fixture");
    let bcp47_strings = tests
        .iter()
        .map(|t| match t.input {
            fixtures::LocaleInfo::Identifier(ref s) => s.identifier.as_str(),
            _ => match t.output {
                fixtures::LocaleInfo::Identifier(ref s) => s.identifier.as_str(),
                _ => panic!("No string in fixture input or output: {t:?}"),
            },
        })
        .collect::<Vec<&str>>();
    for a in bcp47_strings.iter() {
        for b in bcp47_strings.iter() {
            let a_langid = a.parse::<Locale>().expect("Invalid BCP-47 in fixture");
            let a_normalized = a_langid.write_to_string();
            let string_cmp = a_normalized.as_bytes().cmp(b.as_bytes());
            let test_cmp = a_langid.strict_cmp(b.as_bytes());
            assert_eq!(string_cmp, test_cmp, "{a:?}/{b:?}");
        }
    }
}
