//! Tests for normalization.

mod components;
#[macro_use]
mod utils;

#[cfg(feature = "alloc")]
use iri_string::format::ToDedicatedString;
use iri_string::types::*;

use self::components::TEST_CASES;

/// Semantically different IRIs should not be normalized into the same IRI.
#[test]
fn different_iris() {
    for case in TEST_CASES
        .iter()
        .filter(|case| !case.different_iris.is_empty())
    {
        let normalized = IriStr::new(case.normalized_iri).expect("should be valid IRI reference");
        for other in case.different_iris.iter().copied() {
            let other = IriStr::new(other).expect("should be valid IRI reference");
            assert_ne!(
                normalized, other,
                "<{}> should not be normalized to <{other}>, case={case:#?}",
                case.composed
            );
        }
    }
}

/// Normalization should work for IRI.
#[test]
fn normalize_uri() {
    for case in TEST_CASES
        .iter()
        .filter(|case| case.is_uri_class() && case.is_absolute())
    {
        let source = UriStr::new(case.composed).expect("should be valid URI");
        let normalized = source.normalize();
        let expected = UriStr::new(case.normalized_uri).expect("should be valid URI");

        assert_eq_display!(normalized, expected, "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_string(), expected.as_str(), "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_dedicated_string(), expected, "case={case:#?}");

        assert_eq!(
            case.is_rfc3986_normalizable(),
            normalized.ensure_rfc3986_normalizable().is_ok(),
            "case={case:#?}"
        );
    }
}

/// Normalization should work for IRI.
#[test]
fn normalize_iri() {
    for case in TEST_CASES
        .iter()
        .filter(|case| case.is_iri_class() && case.is_absolute())
    {
        let source = IriStr::new(case.composed).expect("should be valid IRI");
        let normalized = source.normalize();
        let expected = IriStr::new(case.normalized_iri).expect("should be valid IRI");

        assert_eq_display!(normalized, expected, "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_string(), expected.as_str(), "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_dedicated_string(), expected, "case={case:#?}");

        assert_eq!(
            case.is_rfc3986_normalizable(),
            normalized.ensure_rfc3986_normalizable().is_ok(),
            "case={case:#?}"
        );
    }
}

/// WHATWG-like normalization should work for IRI.
#[test]
fn normalize_uri_whatwg_like() {
    for case in TEST_CASES
        .iter()
        .filter(|case| case.is_uri_class() && case.is_absolute())
    {
        let source = UriStr::new(case.composed).expect("should be valid URI");
        let normalized = source.normalize_but_preserve_authorityless_relative_path();
        let expected = UriStr::new(
            case.normalized_uri_whatwg_like
                .unwrap_or(case.normalized_uri),
        )
        .expect("should be valid URI");

        assert_eq_display!(normalized, expected, "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_string(), expected.as_str(), "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_dedicated_string(), expected, "case={case:#?}");

        assert_eq!(
            case.is_rfc3986_normalizable(),
            normalized.ensure_rfc3986_normalizable().is_ok(),
            "case={case:#?}"
        );
    }
}

/// WHATWG-like normalization should work for IRI.
#[test]
fn normalize_iri_whatwg_like() {
    for case in TEST_CASES
        .iter()
        .filter(|case| case.is_iri_class() && case.is_absolute())
    {
        let source = IriStr::new(case.composed).expect("should be valid IRI");
        let normalized = source.normalize_but_preserve_authorityless_relative_path();
        let expected = IriStr::new(
            case.normalized_iri_whatwg_like
                .unwrap_or(case.normalized_iri),
        )
        .expect("should be valid IRI");

        assert_eq_display!(normalized, expected, "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_string(), expected.as_str(), "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(normalized.to_dedicated_string(), expected, "case={case:#?}");

        assert_eq!(
            case.is_rfc3986_normalizable(),
            normalized.ensure_rfc3986_normalizable().is_ok(),
            "case={case:#?}"
        );
    }
}

/// Normalization should be idempotent.
#[test]
fn normalize_idempotent() {
    let mut buf = [0_u8; 512];

    for case in TEST_CASES
        .iter()
        .filter(|case| case.is_iri_class() && case.is_absolute())
    {
        let source = IriStr::new(case.composed).expect("should be valid IRI");
        let normalized = source.normalize();
        let expected = IriStr::new(case.normalized_iri).expect("should be valid IRI");

        let normalized_s =
            iri_string::format::write_to_slice(&mut buf, &normalized).expect("not enough buffer");
        let normalized_s = IriStr::new(normalized_s).expect("should be valid IRI reference");

        // Normalize again.
        let normalized_again = normalized_s.normalize();
        assert_eq_display!(normalized_again, expected, "case={case:#?}");
    }
}

/// Normalizedness checks.
#[test]
fn normalizedness() {
    #[derive(Debug, Clone, Copy)]
    struct Case {
        iri: &'static str,
        is_normalized_default: bool,
        is_normalized_rfc3986: bool,
        is_normalized_whatwg_like: bool,
    }
    const CASES: &[Case] = &[
        Case {
            iri: "scheme:/.//foo",
            is_normalized_default: true,
            is_normalized_rfc3986: false,
            is_normalized_whatwg_like: true,
        },
        Case {
            iri: "scheme:.///foo",
            is_normalized_default: false,
            is_normalized_rfc3986: false,
            is_normalized_whatwg_like: true,
        },
        Case {
            iri: "scheme://authority/.//foo",
            is_normalized_default: false,
            is_normalized_rfc3986: false,
            is_normalized_whatwg_like: false,
        },
        Case {
            iri: "scheme:relative/..//foo",
            is_normalized_default: false,
            is_normalized_rfc3986: false,
            is_normalized_whatwg_like: true,
        },
    ];

    for case in CASES {
        let iri = IriStr::new(case.iri).expect("should be valid IRI");
        assert_eq!(
            iri.is_normalized(),
            case.is_normalized_default,
            "case={case:?}"
        );
        assert_eq!(
            iri.is_normalized_rfc3986(),
            case.is_normalized_rfc3986,
            "case={case:?}"
        );
        assert_eq!(
            iri.is_normalized_but_authorityless_relative_path_preserved(),
            case.is_normalized_whatwg_like,
            "case={case:?}"
        );
    }
}
