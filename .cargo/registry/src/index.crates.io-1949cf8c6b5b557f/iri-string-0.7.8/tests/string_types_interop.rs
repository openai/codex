//! Conversions between types.

use iri_string::types::*;

fn assert_convertible<T>(source: &str)
where
    T: ?Sized + PartialEq<str> + core::fmt::Debug,
    for<'a> &'a T: TryFrom<&'a str>,
    for<'a> <&'a T as TryFrom<&'a str>>::Error: core::fmt::Debug,
{
    match <&T>::try_from(source) {
        Ok(parsed) => assert_eq!(parsed, source),
        Err(e) => panic!("should be convertible: source={:?}: {:?}", source, e),
    }
}

fn assert_non_convertible<T>(source: &str)
where
    T: ?Sized + PartialEq<str> + core::fmt::Debug,
    for<'a> &'a T: TryFrom<&'a str>,
    for<'a> <&'a T as TryFrom<&'a str>>::Error: core::fmt::Debug,
{
    if let Ok(parsed) = <&T>::try_from(source) {
        panic!(
            "should not be convertible: source={:?}, parsed={:?}",
            source, parsed
        );
    }
}

#[test]
fn rfc3986_uris_absolute_without_fragment() {
    const URIS: &[&str] = &[
        // RFC 3986 itself.
        "https://tools.ietf.org/html/rfc3986",
        "https://datatracker.ietf.org/doc/html/rfc3986",
        // RFC 3986 section 1.1.2.
        "ftp://ftp.is.co.za/rfc/rfc1808.txt",
        "http://www.ietf.org/rfc/rfc2396.txt",
        "ldap://[2001:db8::7]/c=GB?objectClass?one",
        "mailto:John.Doe@example.com",
        "news:comp.infosystems.www.servers.unix",
        "tel:+1-816-555-1212",
        "telnet://192.0.2.16:80/",
        "urn:oasis:names:specification:docbook:dtd:xml:4.1.2",
        // RFC 3986 section 3.
        "urn:example:animal:ferret:nose",
        // RFC 3986 section 3.3.
        "mailto:fred@example.com",
        "foo://info.example.com?fred",
        // RFC 3986 section 5.4.
        "http://a/b/c/d;p?q",
        // RFC 3986 section 5.4.1.
        "g:h",
        "http://a/b/c/g",
        "http://a/b/c/g/",
        "http://a/g",
        "http://g",
        "http://a/b/c/d;p?y",
        "http://a/b/c/g?y",
        "http://a/b/c/;x",
        "http://a/b/c/g;x",
        "http://a/b/c/d;p?q",
        "http://a/b/c/",
        "http://a/b/",
        "http://a/b/g",
        "http://a/",
        // RFC 3986 section 5.4.2.
        "http://a/b/c/g.",
        "http://a/b/c/.g",
        "http://a/b/c/g..",
        "http://a/b/c/..g",
        "http://a/b/c/g/h",
        "http://a/b/c/h",
        "http://a/b/c/g;x=1/y",
        "http://a/b/c/y",
        "http://a/b/c/g?y/./x",
        "http://a/b/c/g?y/../x",
        // RFC 3986 section 6.2.2.
        "example://a/b/c/%7Bfoo%7D",
        "eXAMPLE://a/./b/../b/%63/%7bfoo%7d",
        // RFC 3986 section 6.2.2.1.
        "HTTP://www.EXAMPLE.com/",
        "http://www.example.com/",
        // RFC 3986 section 6.2.3.
        "http://example.com",
        "http://example.com/",
        "http://example.com:/",
        "http://example.com:80/",
        "http://example.com/?",
        "mailto:Joe@Example.COM",
        "mailto:Joe@example.com",
        // RFC 3986 section 6.2.4.
        "http://example.com/data",
        "http://example.com/data/",
        "ftp://cnn.example.com&story=breaking_news@10.0.0.1/top_story.htm",
        // RFC 3986 section Appendix C.
        "http://www.w3.org/Addressing/",
        "ftp://foo.example.com/rfc/",
        // RFC 3987 itself.
        "https://tools.ietf.org/html/rfc3987",
        "https://datatracker.ietf.org/doc/html/rfc3987",
        // RFC 3987 section 3.1.
        "http://xn--rsum-bpad.example.org",
        "http://r%C3%A9sum%C3%A9.example.org",
        // RFC 3987 section 3.2.
        "http://example.com/%F0%90%8C%80%F0%90%8C%81%F0%90%8C%82",
        // RFC 3987 section 3.2.1.
        "http://www.example.org/r%C3%A9sum%C3%A9.html",
        "http://www.example.org/r%E9sum%E9.html",
        "http://www.example.org/D%C3%BCrst",
        "http://www.example.org/D%FCrst",
        "http://xn--99zt52a.example.org/%e2%80%ae",
        "http://xn--99zt52a.example.org/%E2%80%AE",
        // RFC 3987 section 4.4.
        "http://ab.CDEFGH.ij/kl/mn/op.html",
        "http://ab.CDE.FGH/ij/kl/mn/op.html",
        "http://AB.CD.ef/gh/IJ/KL.html",
        "http://ab.cd.EF/GH/ij/kl.html",
        "http://ab.CD.EF/GH/IJ/kl.html",
        "http://ab.CDE123FGH.ij/kl/mn/op.html",
        "http://ab.cd.ef/GH1/2IJ/KL.html",
        "http://ab.cd.ef/GH%31/%32IJ/KL.html",
        "http://ab.CDEFGH.123/kl/mn/op.html",
        // RFC 3987 section 5.3.2.
        "eXAMPLE://a/./b/../b/%63/%7bfoo%7d/ros%C3%A9",
        // RFC 3987 section 5.3.2.1.
        "HTTP://www.EXAMPLE.com/",
        "http://www.example.com/",
        // RFC 3987 section 5.3.2.3.
        "http://example.org/~user",
        "http://example.org/%7euser",
        "http://example.org/%7Euser",
        // RFC 3987 section 5.3.3.
        "http://example.com",
        "http://example.com/",
        "http://example.com:/",
        "http://example.com:80/",
        //"http://xn--rsum-bpad.example.org",  // duplicate
        // RFC 3987 section 5.3.4.
        "http://example.com/data",
        "http://example.com/data/",
        // RFC 3987 section 6.4.
        //"http://www.example.org/r%C3%A9sum%C3%A9.html",  // duplicate
        //"http://www.example.org/r%E9sum%E9.html",  // duplicate
    ];
    for uri in URIS {
        assert_convertible::<IriReferenceStr>(uri);
        assert_convertible::<UriReferenceStr>(uri);
        assert_convertible::<IriStr>(uri);
        assert_convertible::<UriStr>(uri);
        assert_convertible::<IriAbsoluteStr>(uri);
        assert_convertible::<UriAbsoluteStr>(uri);
        assert_non_convertible::<IriRelativeStr>(uri);
        assert_non_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn rfc3986_uris_absolute_with_fragment() {
    const URIS: &[&str] = &[
        // RFC 3986 section 3.
        "foo://example.com:8042/over/there?name=ferret#nose",
        // RFC 3986 section 5.4.1.
        "http://a/b/c/d;p?q#s",
        "http://a/b/c/g#s",
        "http://a/b/c/g?y#s",
        "http://a/b/c/g;x?y#s",
        // RFC 3986 section 5.4.2.
        "http://a/b/c/g#s/./x",
        "http://a/b/c/g#s/../x",
        // RFC 3986 section Appendix B.
        "http://www.ics.uci.edu/pub/ietf/uri/#Related",
        // RFC 3986 section Appendix C.
        "http://www.ics.uci.edu/pub/ietf/uri/historical.html#WARNING",
        // RFC 3987 section 3.1.
        "http://www.example.org/red%09ros%C3%A9#red",
        // RFC 3987 section 4.4.
        "http://AB.CD.EF/GH/IJ/KL?MN=OP;QR=ST#UV",
    ];
    for uri in URIS {
        assert_convertible::<IriReferenceStr>(uri);
        assert_convertible::<UriReferenceStr>(uri);
        assert_convertible::<IriStr>(uri);
        assert_convertible::<UriStr>(uri);
        assert_non_convertible::<IriAbsoluteStr>(uri);
        assert_non_convertible::<UriAbsoluteStr>(uri);
        assert_non_convertible::<IriRelativeStr>(uri);
        assert_non_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn rfc3986_uris_relative() {
    const URIS: &[&str] = &[
        // RFC 3986 section 5.4.1.
        "g",
        "./g",
        "g/",
        "/g",
        "//g",
        "?y",
        "g?y",
        "#s",
        "g#s",
        "g?y#s",
        ";x",
        "g;x",
        "g;x?y#s",
        "",
        ".",
        "./",
        "..",
        "../",
        "../g",
        "../..",
        "../../",
        "../../g",
        // RFC 3986 section 5.4.2.
        "/./g",
        "/../g",
        "g.",
        ".g",
        "g..",
        "..g",
        "./../g",
        "./g/.",
        "g/./h",
        "g/../h",
        "g;x=1/./y",
        "g;x=1/../y",
        "g?y/./x",
        "g?y/../x",
        "g#s/./x",
        "g#s/../x",
    ];
    for uri in URIS {
        assert_convertible::<IriReferenceStr>(uri);
        assert_convertible::<UriReferenceStr>(uri);
        assert_non_convertible::<IriStr>(uri);
        assert_non_convertible::<UriStr>(uri);
        assert_non_convertible::<IriAbsoluteStr>(uri);
        assert_non_convertible::<UriAbsoluteStr>(uri);
        assert_convertible::<IriRelativeStr>(uri);
        assert_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn rfc3987_iris_absolute_without_fragment() {
    const URIS: &[&str] = &[
        // RFC 3987 section 3.1.
        "http://r\u{E9}sum\u{E9}.example.org",
        // RFC 3987 section 3.2.
        "http://example.com/\u{10300}\u{10301}\u{10302}",
        "http://www.example.org/D\u{FC}rst",
        "http://\u{7D0D}\u{8C46}.example.org/%E2%80%AE",
        // RFC 3987 section 5.2.
        "http://example.org/ros\u{E9}",
        // RFC 3987 section 5.3.2.
        "example://a/b/c/%7Bfoo%7D/ros\u{E9}",
        // RFC 3987 section 5.3.2.2.
        "http://www.example.org/r\u{E9}sum\u{E9}.html",
        "http://www.example.org/re\u{301}sume\u{301}.html",
        // RFC 3987 section 5.3.3.
        //"http://r\u{E9}sum\u{E9}.example.org",  // duplicate
        // RFC 3987 section 6.4.
        //"http://www.example.org/r\u{E9}sum\u{E9}.html",  // duplicate
    ];
    for uri in URIS {
        assert_convertible::<IriReferenceStr>(uri);
        assert_non_convertible::<UriReferenceStr>(uri);
        assert_convertible::<IriStr>(uri);
        assert_non_convertible::<UriStr>(uri);
        assert_convertible::<IriAbsoluteStr>(uri);
        assert_non_convertible::<UriAbsoluteStr>(uri);
        assert_non_convertible::<IriRelativeStr>(uri);
        assert_non_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn rfc3987_iris_absolute_with_fragment() {
    const URIS: &[&str] = &[
        // RFC 3987 section 6.4.
        "http://www.example.org/r%E9sum%E9.xml#r\u{E9}sum\u{E9}",
    ];
    for uri in URIS {
        assert_convertible::<IriReferenceStr>(uri);
        assert_non_convertible::<UriReferenceStr>(uri);
        assert_convertible::<IriStr>(uri);
        assert_non_convertible::<UriStr>(uri);
        assert_non_convertible::<IriAbsoluteStr>(uri);
        assert_non_convertible::<UriAbsoluteStr>(uri);
        assert_non_convertible::<IriRelativeStr>(uri);
        assert_non_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn test_invalid_char() {
    const URIS: &[&str] = &[
        "##", // Fragment cannot have `#`.
        "<",  // `<` cannot appear in an IRI reference.
        ">",  // `>` cannot appear in an IRI reference.
    ];
    for uri in URIS {
        assert_non_convertible::<IriReferenceStr>(uri);
        assert_non_convertible::<UriReferenceStr>(uri);
        assert_non_convertible::<IriStr>(uri);
        assert_non_convertible::<UriStr>(uri);
        assert_non_convertible::<IriAbsoluteStr>(uri);
        assert_non_convertible::<UriAbsoluteStr>(uri);
        assert_non_convertible::<IriRelativeStr>(uri);
        assert_non_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn invalid_percent_encoding() {
    const URIS: &[&str] = &["%", "%0", "%0g", "%f", "%fg", "%g", "%g0", "%gf", "%gg"];
    for uri in URIS {
        assert_non_convertible::<IriReferenceStr>(uri);
        assert_non_convertible::<UriReferenceStr>(uri);
        assert_non_convertible::<IriStr>(uri);
        assert_non_convertible::<UriStr>(uri);
        assert_non_convertible::<IriAbsoluteStr>(uri);
        assert_non_convertible::<UriAbsoluteStr>(uri);
        assert_non_convertible::<IriRelativeStr>(uri);
        assert_non_convertible::<UriRelativeStr>(uri);
    }
}

#[test]
fn compare_different_types()
where
    UriAbsoluteStr: PartialEq<IriReferenceStr>,
    IriReferenceStr: PartialEq<UriAbsoluteStr>,
    IriAbsoluteStr: PartialEq<UriReferenceStr>,
    UriReferenceStr: PartialEq<IriAbsoluteStr>,
{
}
