// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

mod fixtures;

use std::convert::TryInto;
use writeable::*;

use icu_locale_core::{subtags, LanguageIdentifier, ParseError};

type Result = std::result::Result<LanguageIdentifier, ParseError>;

fn test_langid_fixtures(tests: Vec<fixtures::LocaleTest>) {
    for test in tests {
        match test.output {
            fixtures::LocaleInfo::String(s) => {
                if let fixtures::LocaleInfo::Object(ref o) = &test.input {
                    if o.field_type == "Locale" {
                        continue;
                    }
                }
                let input: LanguageIdentifier = test.input.try_into().expect("Parsing failed.");
                assert_writeable_eq!(input, s);
            }
            fixtures::LocaleInfo::Error(err) => {
                let err: ParseError = err.into();
                let input: Result = test.input.try_into();
                assert_eq!(input, Err(err));
            }
            fixtures::LocaleInfo::Identifier(ident) => {
                let input: LanguageIdentifier = test.input.try_into().expect("Parsing failed.");
                let output: LanguageIdentifier = ident.try_into().expect("Parsing failed.");
                assert_eq!(input, output);
            }
            fixtures::LocaleInfo::Object(o) => {
                let input: LanguageIdentifier = test.input.try_into().expect("Parsing failed.");
                let output: LanguageIdentifier = o.try_into().expect("Parsing failed.");
                assert_eq!(input, output);
            }
        }
    }
}

#[test]
fn test_langid_parsing() {
    let data = serde_json::from_str(include_str!("fixtures/langid.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_langid_invalid() {
    let data = serde_json::from_str(include_str!("fixtures/invalid.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_langid_canonicalize() {
    let data = serde_json::from_str(include_str!("fixtures/canonicalize.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_langid_from_locale() {
    let data = serde_json::from_str(include_str!("fixtures/locale.json"))
        .expect("Failed to read a fixture");

    test_langid_fixtures(data);
}

#[test]
fn test_langid_subtag_language() {
    let mut lang: subtags::Language = "en".parse().expect("Failed to parse a language.");
    assert_eq!(lang.as_str(), "en");

    lang = subtags::Language::UNKNOWN;
    assert!(lang.is_unknown());

    assert_writeable_eq!(lang, "und");
}

#[test]
fn test_langid_subtag_region() {
    let region: subtags::Region = "en".parse().expect("Failed to parse a region.");
    assert_eq!(region.as_str(), "EN");
    assert_writeable_eq!(region, "EN");
}

#[test]
fn test_langid_subtag_script() {
    let script: subtags::Script = "Latn".parse().expect("Failed to parse a script.");
    assert_eq!(script.as_str(), "Latn");
    assert_writeable_eq!(script, "Latn");
}

#[test]
fn test_langid_subtag_variant() {
    let variant: subtags::Variant = "macos".parse().expect("Failed to parse a variant.");
    assert_eq!(variant.as_str(), "macos");
    assert_writeable_eq!(variant, "macos");
}

#[test]
fn test_langid_subtag_variants() {
    let variant: subtags::Variant = "macos".parse().expect("Failed to parse a variant.");
    let mut variants = subtags::Variants::from_vec_unchecked(vec![variant]);
    assert_eq!(variants.first(), Some(&variant));
    variants.clear();
    assert_eq!(variants.len(), 0);
}

#[test]
fn test_langid_normalizing_eq_str() {
    let tests: Vec<fixtures::LocaleTest> =
        serde_json::from_str(include_str!("fixtures/langid.json"))
            .expect("Failed to read a fixture");
    for test in tests {
        let parsed: LanguageIdentifier = test.input.try_into().expect("Parsing failed.");
        assert!(parsed.normalizing_eq(&parsed.write_to_string()));
    }

    // Check that trailing characters are not ignored
    let lang: LanguageIdentifier = "en".parse().expect("Parsing failed.");
    assert!(!lang.normalizing_eq("en-US"));
}

#[test]
fn test_langid_strict_cmp() {
    let tests: Vec<fixtures::LocaleTest> =
        serde_json::from_str(include_str!("fixtures/langid.json"))
            .expect("Failed to read a fixture");
    let bcp47_strings = tests
        .iter()
        .map(|t| match t.input {
            fixtures::LocaleInfo::String(ref s) => s.as_str(),
            _ => panic!("Invalid fixture"),
        })
        .collect::<Vec<&str>>();
    for a in bcp47_strings.iter() {
        for b in bcp47_strings.iter() {
            let a_langid = a
                .parse::<LanguageIdentifier>()
                .expect("Invalid BCP-47 in fixture");
            let a_normalized = a_langid.write_to_string();
            let string_cmp = a_normalized.as_bytes().cmp(b.as_bytes());
            let test_cmp = a_langid.strict_cmp(b.as_bytes());
            assert_eq!(string_cmp, test_cmp, "{a:?}/{b:?}");
        }
    }
}
