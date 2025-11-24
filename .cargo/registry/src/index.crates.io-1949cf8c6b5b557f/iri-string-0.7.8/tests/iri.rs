//! Tests specific to IRIs (not URIs).

#[macro_use]
mod utils;

use iri_string::format::write_to_slice;
#[cfg(feature = "alloc")]
use iri_string::format::ToDedicatedString;
#[cfg(feature = "alloc")]
use iri_string::types::IriReferenceString;
use iri_string::types::{IriReferenceStr, UriReferenceStr};

#[derive(Debug, Clone, Copy)]
struct TestCase {
    iri: &'static str,
    uri: &'static str,
}

// `[(iri, uri)]`.
const CASES: &[TestCase] = &[
    TestCase {
        iri: "?alpha=\u{03B1}",
        uri: "?alpha=%CE%B1",
    },
    TestCase {
        iri: "?katakana-letter-i=\u{30A4}",
        uri: "?katakana-letter-i=%E3%82%A4",
    },
    TestCase {
        iri: "?sushi=\u{1f363}",
        uri: "?sushi=%F0%9F%8D%A3",
    },
];

#[test]
fn iri_to_uri() {
    let mut buf = [0_u8; 256];
    let mut buf2 = [0_u8; 256];

    for case in CASES.iter().copied() {
        let expected = UriReferenceStr::new(case.uri).expect("should be valid URI reference");

        let iri = IriReferenceStr::new(case.iri).expect("should be valid URI reference");
        let encoded = iri.encode_to_uri();
        assert_eq_display!(encoded, expected);
        let encoded_uri = write_to_slice(&mut buf, &encoded).expect("not enough buffer");
        let encoded_uri = UriReferenceStr::new(encoded_uri).expect("should be valid URI reference");
        assert_eq!(encoded_uri, expected);

        let encoded_again = AsRef::<IriReferenceStr>::as_ref(encoded_uri).encode_to_uri();
        assert_eq_display!(encoded_again, expected);
        let encoded_again_uri =
            write_to_slice(&mut buf2, &encoded_again).expect("not enough buffer");
        let encoded_again_uri =
            UriReferenceStr::new(encoded_again_uri).expect("should be valid URI reference");
        assert_eq!(encoded_again_uri, expected);
    }
}

#[cfg(feature = "alloc")]
#[test]
fn iri_to_uri_allocated() {
    for case in CASES.iter().copied() {
        let expected = UriReferenceStr::new(case.uri).expect("should be valid URI reference");

        let iri = IriReferenceStr::new(case.iri).expect("should be valid URI reference");
        let encoded = iri.encode_to_uri().to_dedicated_string();
        assert_eq!(encoded, expected);

        let encoded_again = AsRef::<IriReferenceStr>::as_ref(&encoded)
            .encode_to_uri()
            .to_dedicated_string();
        assert_eq!(encoded_again, expected);
    }
}

#[cfg(feature = "alloc")]
#[test]
fn iri_to_uri_inline() {
    for case in CASES.iter().copied() {
        let expected = UriReferenceStr::new(case.uri).expect("should be valid URI reference");

        let mut iri =
            IriReferenceString::try_from(case.iri).expect("should be valid URI reference");

        iri.encode_to_uri_inline();
        assert_eq!(iri, expected);

        iri.encode_to_uri_inline();
        assert_eq!(
            iri, expected,
            "``encode_to_uri_inline()` method should be idempotent"
        );
    }
}
