//! Test cases for issues reported on GitHub.

#[macro_use]
mod utils;

use iri_string::types::UriReferenceStr;

mod issue_17 {
    use super::*;

    #[test]
    fn ipv6_literal_authority_host() {
        let uri = UriReferenceStr::new("//[::1]").expect("valid relative URI");
        let authority = uri
            .authority_components()
            .expect("the URI has authority `[::1]`");
        assert_eq!(authority.host(), "[::1]");
    }

    #[test]
    fn extra_trailing_colon_in_ipv6_literal() {
        assert!(UriReferenceStr::new("//[::1:]").is_err());
    }

    #[test]
    fn ipvfuture_literal_capital_v() {
        assert!(UriReferenceStr::new("//[v0.0]").is_ok());
        assert!(UriReferenceStr::new("//[V0.0]").is_ok());
    }

    #[test]
    fn ipvfuture_empty_part() {
        assert!(
            UriReferenceStr::new("//[v0.]").is_err(),
            "address should not be empty"
        );
        assert!(
            UriReferenceStr::new("//[v.0]").is_err(),
            "version should not be empty"
        );
        assert!(
            UriReferenceStr::new("//[v.]").is_err(),
            "neither address nor version should be empty"
        );
    }
}

mod issue_36 {
    use super::*;

    #[cfg(feature = "alloc")]
    use iri_string::format::ToDedicatedString;
    use iri_string::types::UriAbsoluteStr;

    // "/.//.".resolve_against("a:/")
    // => "a:" + remove_dot_segments("/.//.")
    //
    // STEP     OUTPUT BUFFER   INPUT BUFFER
    //  1 :                     /.//.
    //  2B:                     //.
    //  2E:     /               /.
    //  2B:     /               /
    //  2E:     //
    //  (see RFC 3986 section 5.2.4 for this notation.)
    //
    // => "a://"
    //
    // However, this is invalid since it should be semantically
    // `<scheme="a">:<path="//">` but this string will be parsed as
    // `<scheme="a">://<path="">`. So, `./` should be inserted to break
    // `//` at the beginning of the path part.
    #[test]
    fn abnormal_resolution() {
        let base = UriAbsoluteStr::new("a:/").expect("valid absolute URI");
        {
            let relative = UriReferenceStr::new("/.//.").expect("valid relative URI");
            let result = relative.resolve_against(base);

            assert!(
                result.ensure_rfc3986_normalizable().is_err(),
                "strict RFC 3986 resolution should fail for base={:?}, ref={:?}",
                base,
                relative
            );
            assert_eq_display!(
                result,
                "a:/.//",
                "resolution result will be modified using serialization by WHATWG URL Standard"
            );
        }
        {
            let relative = UriReferenceStr::new(".//.").expect("valid relative URI");
            let result = relative.resolve_against(base);

            assert!(
                result.ensure_rfc3986_normalizable().is_err(),
                "strict RFC 3986 resolution should fail for base={:?}, ref={:?}",
                base,
                relative
            );
            assert_eq_display!(
                result,
                "a:/.//",
                "resolution result will be modified using serialization by WHATWG URL Standard"
            );
        }
    }

    #[test]
    fn abnormal_normalization() {
        let uri = UriAbsoluteStr::new("a:/.//.").expect("valid absolute URI");

        let normalized = uri.normalize();
        assert!(
            normalized.ensure_rfc3986_normalizable().is_err(),
            "strict RFC 3986 normalization should fail for uri={:?}",
            uri
        );
        assert_eq_display!(
            normalized,
            "a:/.//",
            "normalization result will be modified using serialization by WHATWG URL Standard"
        );

        #[cfg(feature = "alloc")]
        {
            assert!(
                !normalized.to_dedicated_string().is_normalized_rfc3986(),
                "not normalizable by strict RFC 3986 algorithm"
            );
        }
    }

    #[test]
    fn abnormal_normalization2() {
        {
            let uri = UriAbsoluteStr::new("a:/bar//.").expect("valid absolute URI");
            assert_eq_display!(uri.normalize(), "a:/bar//");
        }
        {
            let uri = UriAbsoluteStr::new("a:/bar/..//.").expect("valid absolute URI");
            assert_eq_display!(
                uri.normalize(),
                "a:/.//",
                "normalization result will be modified using serialization by WHATWG URL Standard"
            );
        }
        {
            let uri = UriAbsoluteStr::new("a:/.//bar/.").expect("valid absolute URI");
            assert_eq_display!(
                uri.normalize(),
                "a:/.//bar/",
                "normalization result will be modified using serialization by WHATWG URL Standard"
            );
        }
        {
            let uri = UriAbsoluteStr::new("a:/././././././foo/./.././././././././././/.")
                .expect("valid absolute URI");
            assert_eq_display!(
                uri.normalize(),
                "a:/.//",
                "normalization result will be modified using serialization by WHATWG URL Standard"
            );
        }
    }

    #[test]
    fn normalization_pct_triplet_loss() {
        let uri = UriAbsoluteStr::new("a://%92%99").expect("valid absolute URI");
        assert_eq_display!(uri.normalize(), "a://%92%99");
        // Other problems are found during fixing this bug. The test cases for
        // them have been added to generic test case data source.
    }
}

/// <https://github.com/lo48576/iri-string/pull/46>
#[cfg(feature = "alloc")]
mod issue_46 {
    use iri_string::types::{UriFragmentStr, UriRelativeString};

    #[test]
    fn set_fragment_to_relative() {
        let mut uri =
            UriRelativeString::try_from("//user:password@example.com/path?query#frag.old")
                .expect("valid relative URI");
        assert_eq!(uri, "//user:password@example.com/path?query#frag.old");
        assert_eq!(uri.fragment_str(), Some("frag.old"));

        uri.set_fragment(None);
        assert_eq!(uri, "//user:password@example.com/path?query");
        assert_eq!(uri.fragment(), None);

        let frag_new = UriFragmentStr::new("frag-new").expect("valid URI fragment");
        uri.set_fragment(Some(frag_new));
        assert_eq!(uri.fragment_str(), Some("frag-new"));
    }
}
