//! Serde test.
#![cfg(feature = "serde")]

use serde_test::{assert_tokens, Token};

use iri_string::types::*;

mod utils;

macro_rules! define_tests {
    ($positive:ident, $negative:ident, ($spec:ident, $kind:ident), $slice:ty, $owned:ty,) => {
        define_tests! {
            @positive,
            $positive,
            ($spec, $kind),
            $slice,
            $owned,
        }
    };
    (@positive, $name:ident, ($spec:ident, $kind:ident), $slice:ty, $owned:ty,) => {
        #[test]
        fn $name() {
            for raw in utils::positive(utils::Spec::$spec, utils::Kind::$kind) {
                let s = <$slice>::new(raw).expect("Should not fail: valid string");
                assert_tokens(&s, &[Token::BorrowedStr(raw)]);

                #[cfg(all(feature = "serde", feature = "alloc"))]
                {
                    let s = s.to_owned();
                    assert_tokens(&s, &[Token::BorrowedStr(raw)]);
                }
            }
        }
    };
}

define_tests! {
    uri,
    not_uri,
    (Uri, Normal),
    UriStr,
    UriString,
}

define_tests! {
    uri_absolute,
    not_uri_absolute,
    (Uri, Absolute),
    UriAbsoluteStr,
    UriAbsoluteString,
}

define_tests! {
    uri_reference,
    not_uri_reference,
    (Uri, Reference),
    UriReferenceStr,
    UriReferenceString,
}

define_tests! {
    uri_relative,
    not_uri_relative,
    (Uri, Relative),
    UriRelativeStr,
    UriRelativeString,
}

define_tests! {
    iri,
    not_iri,
    (Iri, Normal),
    IriStr,
    IriString,
}

define_tests! {
    iri_absolute,
    not_iri_absolute,
    (Iri, Absolute),
    IriAbsoluteStr,
    IriAbsoluteString,
}

define_tests! {
    iri_reference,
    not_iri_reference,
    (Iri, Reference),
    IriReferenceStr,
    IriReferenceString,
}

define_tests! {
    iri_relative,
    not_iri_relative,
    (Iri, Relative),
    IriRelativeStr,
    IriRelativeString,
}
