//! Tests for percent encoding.

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
mod utils;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::ToString;

use iri_string::percent_encode::{PercentEncodedForIri, PercentEncodedForUri};

#[test]
fn regname_uri() {
    let encoded = PercentEncodedForUri::from_reg_name("alpha.\u{03B1}.reg.name");
    let expected = "alpha.%CE%B1.reg.name";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn regname_iri() {
    let encoded = PercentEncodedForIri::from_reg_name("alpha.\u{03B1}.reg.name");
    let expected = "alpha.\u{03B1}.reg.name";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn path_segment_uri() {
    let encoded = PercentEncodedForUri::from_path_segment("\u{03B1}/<alpha>?#");
    let expected = "%CE%B1%2F%3Calpha%3E%3F%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn path_segment_iri() {
    let encoded = PercentEncodedForIri::from_path_segment("\u{03B1}/<alpha>?#");
    let expected = "\u{03B1}%2F%3Calpha%3E%3F%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn path_uri() {
    let encoded = PercentEncodedForUri::from_path("\u{03B1}/<alpha>?#");
    let expected = "%CE%B1/%3Calpha%3E%3F%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn path_iri() {
    let encoded = PercentEncodedForIri::from_path("\u{03B1}/<alpha>?#");
    let expected = "\u{03B1}/%3Calpha%3E%3F%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn query_uri() {
    let encoded = PercentEncodedForUri::from_query("\u{03B1}/<alpha>?#");
    let expected = "%CE%B1/%3Calpha%3E?%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn query_iri() {
    let encoded = PercentEncodedForIri::from_query("\u{03B1}/<alpha>?#");
    let expected = "\u{03B1}/%3Calpha%3E?%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn fragment_uri() {
    let encoded = PercentEncodedForUri::from_fragment("\u{03B1}/<alpha>?#");
    let expected = "%CE%B1/%3Calpha%3E?%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn fragment_iri() {
    let encoded = PercentEncodedForIri::from_fragment("\u{03B1}/<alpha>?#");
    let expected = "\u{03B1}/%3Calpha%3E?%23";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn unreserve_uri_unreserved() {
    let encoded = PercentEncodedForUri::unreserve("%a0-._~\u{03B1}");
    let expected = "%25a0-._~%CE%B1";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn unreserve_iri_unreserved() {
    let encoded = PercentEncodedForIri::unreserve("%a0-._~\u{03B1}");
    let expected = "%25a0-._~\u{03B1}";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn unreserve_uri_reserved() {
    let encoded = PercentEncodedForUri::unreserve(":/?#[]@ !$&'()*+,;=");
    let expected = "%3A%2F%3F%23%5B%5D%40%20%21%24%26%27%28%29%2A%2B%2C%3B%3D";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn unreserve_iri_reserved() {
    let encoded = PercentEncodedForIri::unreserve(":/?#[]@ !$&'()*+,;=");
    let expected = "%3A%2F%3F%23%5B%5D%40%20%21%24%26%27%28%29%2A%2B%2C%3B%3D";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn characters_uri_unreserved() {
    let encoded = PercentEncodedForUri::characters("%a0-._~\u{03B1}");
    let expected = "%25a0-._~%CE%B1";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn characters_iri_unreserved() {
    let encoded = PercentEncodedForIri::characters("%a0-._~\u{03B1}");
    let expected = "%25a0-._~\u{03B1}";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn characters_uri_reserved() {
    let encoded = PercentEncodedForUri::characters(":/?#[]@ !$&'()*+,;=");
    let expected = ":/?#[]@%20!$&'()*+,;=";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}

#[test]
fn characters_iri_reserved() {
    let encoded = PercentEncodedForIri::characters(":/?#[]@ !$&'()*+,;=");
    let expected = ":/?#[]@%20!$&'()*+,;=";
    assert_eq_display!(encoded, expected);
    #[cfg(feature = "alloc")]
    assert_eq!(encoded.to_string(), expected);
}
