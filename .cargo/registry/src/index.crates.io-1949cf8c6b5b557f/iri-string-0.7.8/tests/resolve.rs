//! Tests for IRI resolution.

mod components;
#[macro_use]
mod utils;
mod resolve_refimpl;

use iri_string::format::write_to_slice;
#[cfg(feature = "alloc")]
use iri_string::format::ToDedicatedString;
use iri_string::resolve::FixedBaseResolver;
use iri_string::types::*;

#[cfg(feature = "alloc")]
use self::resolve_refimpl::resolve as resolve_refimpl;

/// Test cases for strict resolvers.
// [(base, [(reference, output, Option<output_normalized>)])]
#[allow(clippy::type_complexity)]
const TEST_CASES: &[(&str, &[(&str, &str, Option<&str>)])] = &[
    // RFC 3986, section 5.2.4.
    ("scheme:///a/b/c/./../../", &[("g", "scheme:///a/g", None)]),
    ("scheme:///a/b/c/./../", &[("../g", "scheme:///a/g", None)]),
    ("scheme:///a/b/c/./", &[("../../g", "scheme:///a/g", None)]),
    ("scheme:///a/b/c/", &[("./../../g", "scheme:///a/g", None)]),
    ("scheme:///a/b/", &[("c/./../../g", "scheme:///a/g", None)]),
    ("scheme:///a/", &[("b/c/./../../g", "scheme:///a/g", None)]),
    ("scheme:///", &[("a/b/c/./../../g", "scheme:///a/g", None)]),
    ("scheme:mid/content=5/../", &[("6", "scheme:mid/6", None)]),
    ("scheme:mid/content=5/", &[("../6", "scheme:mid/6", None)]),
    ("scheme:mid/", &[("content=5/../6", "scheme:mid/6", None)]),
    ("scheme:", &[("mid/content=5/../6", "scheme:mid/6", None)]),
    // RFC 3986, section 5.4.1.
    (
        "http://a/b/c/d;p?q",
        &[
            ("g:h", "g:h", None),
            ("g", "http://a/b/c/g", None),
            ("./g", "http://a/b/c/g", None),
            ("g/", "http://a/b/c/g/", None),
            ("/g", "http://a/g", None),
            ("//g", "http://g", None),
            ("?y", "http://a/b/c/d;p?y", None),
            ("g?y", "http://a/b/c/g?y", None),
            ("#s", "http://a/b/c/d;p?q#s", None),
            ("g#s", "http://a/b/c/g#s", None),
            ("g?y#s", "http://a/b/c/g?y#s", None),
            (";x", "http://a/b/c/;x", None),
            ("g;x", "http://a/b/c/g;x", None),
            ("g;x?y#s", "http://a/b/c/g;x?y#s", None),
            ("", "http://a/b/c/d;p?q", None),
            (".", "http://a/b/c/", None),
            ("./", "http://a/b/c/", None),
            ("..", "http://a/b/", None),
            ("../", "http://a/b/", None),
            ("../g", "http://a/b/g", None),
            ("../..", "http://a/", None),
            ("../../", "http://a/", None),
            ("../../g", "http://a/g", None),
        ],
    ),
    // RFC 3986, section 5.4.2.
    (
        "http://a/b/c/d;p?q",
        &[
            ("../../../g", "http://a/g", None),
            ("../../../../g", "http://a/g", None),
            ("/./g", "http://a/g", None),
            ("/../g", "http://a/g", None),
            ("g.", "http://a/b/c/g.", None),
            (".g", "http://a/b/c/.g", None),
            ("g..", "http://a/b/c/g..", None),
            ("..g", "http://a/b/c/..g", None),
            ("./../g", "http://a/b/g", None),
            ("./g/.", "http://a/b/c/g/", None),
            ("g/./h", "http://a/b/c/g/h", None),
            ("g/../h", "http://a/b/c/h", None),
            ("g;x=1/./y", "http://a/b/c/g;x=1/y", None),
            ("g;x=1/../y", "http://a/b/c/y", None),
            ("g?y/./x", "http://a/b/c/g?y/./x", None),
            ("g?y/../x", "http://a/b/c/g?y/../x", None),
            ("g#s/./x", "http://a/b/c/g#s/./x", None),
            ("g#s/../x", "http://a/b/c/g#s/../x", None),
            ("http:g", "http:g", None),
        ],
    ),
    // Custom cases.
    (
        "http://a/b/c/d/e/../..",
        &[
            // `/a/b/c/d/e/../..` but without dot segments removal.
            ("", "http://a/b/c/d/e/../..", Some("http://a/b/c/")),
            // `/a/b/c/d/e/../..`
            ("..", "http://a/b/c/", None),
            // `/a/b/c/d/e/../../`
            ("../", "http://a/b/c/", None),
            // `/a/b/c/d/e/../.`
            (".", "http://a/b/c/d/", None),
            // `/a/b/c/d/e/.././`
            ("./", "http://a/b/c/d/", None),
            // `/a/b/c/d/e/../..?query` but without dot segments removal.
            (
                "?query",
                "http://a/b/c/d/e/../..?query",
                Some("http://a/b/c/?query"),
            ),
            // `/a/b/c/d/e/../..#frag` but without dot segments removal.
            (
                "#frag",
                "http://a/b/c/d/e/../..#frag",
                Some("http://a/b/c/#frag"),
            ),
            // If the authority is specified, paths won't be merged.
            ("http://example.com", "http://example.com", None),
            ("http://example.com/", "http://example.com/", None),
            // If the path of the reference is not empty, remove_dot_segments is applied.
            ("http://example.com/..", "http://example.com/", None),
            // If the scheme is specified, paths won't be merged.
            ("scheme:", "scheme:", None),
            ("scheme:foo#frag", "scheme:foo#frag", None),
        ],
    ),
    // Custom cases.
    (
        "https://a/b/c",
        &[
            ("", "https://a/b/c", None),
            ("x/", "https://a/b/x/", None),
            ("x//", "https://a/b/x//", None),
            ("x///", "https://a/b/x///", None),
            ("x//y", "https://a/b/x//y", None),
            ("x//y/", "https://a/b/x//y/", None),
            ("x//y//", "https://a/b/x//y//", None),
            // `/b/x//..//y//`.
            // STEP     OUTPUT BUFFER   INPUT BUFFER
            //  1 :                     /b/x//..//y//
            //  2E:     /b              /x//..//y//
            //  2E:     /b/x            //..//y//
            //  2E:     /b/x/           /..//y//
            //  2C:     /b/x            //y//
            //  2E:     /b/x/           /y//
            //  2E:     /b/x//y         //
            //  2E:     /b/x//y/        /
            //  2E:     /b/x//y//
            ("x//..//y//", "https://a/b/x//y//", None),
        ],
    ),
    // Custom cases.
    (
        "scheme:a/b/c",
        &[
            ("", "scheme:a/b/c", None),
            ("x/", "scheme:a/b/x/", None),
            ("x//", "scheme:a/b/x//", None),
            ("x///", "scheme:a/b/x///", None),
            ("x//y", "scheme:a/b/x//y", None),
            ("x//y/", "scheme:a/b/x//y/", None),
            // `a/b/x//..//y//`.
            // STEP     OUTPUT BUFFER   INPUT BUFFER
            //  1 :                     a/b/x//..//y//
            //  2E:     a               /b/x//..//y//
            //  2E:     a/b             /x//..//y//
            //  2E:     a/b/x           //..//y//
            //  2E:     a/b/x/          /..//y//
            //  2C:     a/b/x           //y//
            //  2E:     a/b/x/          /y//
            //  2E:     a/b/x//y        //
            //  2E:     a/b/x//y/       /
            //  2E:     a/b/x//y//
            ("x//..//y//", "scheme:a/b/x//y//", None),
        ],
    ),
    // Custom cases.
    (
        "scheme:a",
        &[
            // `x/../..`.
            // STEP     OUTPUT BUFFER   INPUT BUFFER
            //  1 :                     x/../..
            //  2E:     x               /../..
            //  2C:                     /..
            //  2C:                     /
            //  2E:     /
            ("x/../..", "scheme:/", None),
            // `x/../../y`.
            // STEP     OUTPUT BUFFER   INPUT BUFFER
            //  1 :                     x/../../y
            //  2E:     x               /../../y
            //  2C:                     /../y
            //  2C:                     /y
            //  2E:     /y
            ("x/../../y", "scheme:/y", None),
        ],
    ),
    // Custom cases.
    // Empty base path should be considered as `/` when the base authority is present.
    (
        "scheme://host",
        &[
            ("", "scheme://host", None),
            (".", "scheme://host/", None),
            ("..", "scheme://host/", None),
            ("foo", "scheme://host/foo", None),
        ],
    ),
    // Custom cases.
    (
        "HTTP://USER:PASS@EXAMPLE.COM:80/1/2/3/4/.././5/../6/?QUERY",
        &[(
            "A/b/c/d/e/f/g/h/i/../../../j/k/l/../../../../m/n/./o",
            "HTTP://USER:PASS@EXAMPLE.COM:80/1/2/3/6/A/b/c/d/e/m/n/o",
            Some("http://USER:PASS@example.com:80/1/2/3/6/A/b/c/d/e/m/n/o"),
        )],
    ),
    (
        "HTTP://USER:PASS@EXAMPLE.COM:/%7e/2/beta=%CE%B2/4/.././5/../6/",
        &[(
            "a/b/alpha=%CE%B1/d/e/f/g/h/i/../../../j/k/l/../../../../%3c/%7e/./%3e?query#fragment",
            "HTTP://USER:PASS@EXAMPLE.COM:/%7e/2/beta=%CE%B2/6/a/b/alpha=%CE%B1/d/e/%3c/%7e/%3e?query#fragment",
            Some("http://USER:PASS@example.com/~/2/beta=\u{03B2}/6/a/b/alpha=\u{03B1}/d/e/%3C/~/%3E?query#fragment")
        )],
    ),
    (
        "http://user:pass@example.com:/%7e/2/beta=%ce%b2/4/.././5/../6/",
        &[(
            "a/b/alpha=%ce%b1/d/e/f/g/h/i/../../../j/k/l/../../../../%3c/%7e/./%3e?query#fragment",
            "http://user:pass@example.com:/%7e/2/beta=%ce%b2/6/a/b/alpha=%ce%b1/d/e/%3c/%7e/%3e?query#fragment",
            Some("http://user:pass@example.com/~/2/beta=\u{03B2}/6/a/b/alpha=\u{03B1}/d/e/%3C/~/%3E?query#fragment")
        )],
    ),
];

#[test]
fn resolve() {
    for (base, pairs) in TEST_CASES {
        let base = IriAbsoluteStr::new(base).expect("should be valid base IRI");

        for (target, expected, _normalized_expected) in *pairs {
            let target = IriReferenceStr::new(target).expect("should be valid IRI reference");
            let resolved = target.resolve_against(base);
            assert_eq_display!(resolved, expected, "base={base:?}, target={target:?}");

            #[cfg(feature = "alloc")]
            assert_eq!(
                resolved.to_dedicated_string().as_str(),
                *expected,
                "base={base:?}, target={target:?}"
            );
        }
    }
}

#[test]
fn resolve_normalize() {
    for (base, pairs) in TEST_CASES {
        let base = IriAbsoluteStr::new(base).expect("should be valid base IRI");

        for (target, expected, expected_normalized) in *pairs {
            let target = IriReferenceStr::new(target).expect("should be valid IRI reference");
            let resolved_normalized = target.resolve_against(base).and_normalize();
            let expected = expected_normalized.unwrap_or(*expected);
            assert_eq_display!(
                resolved_normalized,
                expected,
                "base={base:?}, target={target:?}"
            );

            #[cfg(feature = "alloc")]
            assert_eq!(
                resolved_normalized.to_dedicated_string().as_str(),
                expected,
                "base={base:?}, target={target:?}"
            );
        }
    }
}

#[test]
fn fixed_base_resolver() {
    for (base, pairs) in TEST_CASES {
        let base = IriAbsoluteStr::new(base).expect("should be valid base IRI");
        let resolver = FixedBaseResolver::new(base);

        for (target, expected, _normalized_expected) in *pairs {
            let target = IriReferenceStr::new(target).expect("should be valid IRI reference");
            let resolved = resolver.resolve(target);

            assert_eq_display!(resolved, expected, "base={base:?}, target={target:?}");
            #[cfg(feature = "alloc")]
            assert_eq!(
                resolved.to_dedicated_string().as_str(),
                *expected,
                "base={base:?}, target={target:?}"
            );
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn same_result_as_reference_impl() {
    for (base, pairs) in TEST_CASES {
        let base = IriAbsoluteStr::new(base).expect("should be valid base IRI");

        for (target, expected, _normalized_expected) in *pairs {
            let target = IriReferenceStr::new(target).expect("should be valid IRI reference");
            let resolved = target.resolve_against(base).to_dedicated_string();

            let expected_refimpl = resolve_refimpl(target, base);
            assert_eq!(
                *expected, expected_refimpl,
                "base={base:?}, target={target:?}"
            );
            assert_eq!(
                resolved, expected_refimpl,
                "base={base:?}, target={target:?}"
            );
        }
    }
}

#[test]
fn percent_encoded_dots() {
    // [(base, ref, result)]
    const TEST_CASES: &[(&str, &str, &str)] = &[
        ("scheme:", ".", "scheme:"),
        ("scheme:", "%2e", "scheme:"),
        ("scheme:", "%2E", "scheme:"),
        ("scheme://a", ".", "scheme://a/"),
        ("scheme://a", "%2e", "scheme://a/"),
        ("scheme://a", "%2E", "scheme://a/"),
        ("scheme://a/b/c", ".", "scheme://a/b/"),
        ("scheme://a/b/c", "%2e", "scheme://a/b/"),
        ("scheme://a/b/c", "%2E", "scheme://a/b/"),
        ("scheme://a/b/c", "./g", "scheme://a/b/g"),
        ("scheme://a/b/c", "%2e/g", "scheme://a/b/g"),
        ("scheme://a/b/c", "%2E/g", "scheme://a/b/g"),
        ("scheme://a/b/c/d/e/f", "../../../g", "scheme://a/b/g"),
        (
            "scheme://a/b/c/d/e/f",
            "%2E%2E/%2E%2e/%2E./g",
            "scheme://a/b/g",
        ),
        (
            "scheme://a/b/c/d/e/f",
            "%2e%2E/%2e%2e/%2e./g",
            "scheme://a/b/g",
        ),
        ("scheme://a/b/c/d/e/f", ".%2E/.%2e/../g", "scheme://a/b/g"),
    ];

    for (base, reference, expected) in TEST_CASES {
        let base = IriAbsoluteStr::new(base).expect("should be valid base IRI");
        let reference = IriReferenceStr::new(reference).expect("should be valid IRI reference");

        let resolved = reference.resolve_against(base);
        assert_eq_display!(resolved, *expected);
        #[cfg(feature = "alloc")]
        assert_eq!(resolved.to_dedicated_string(), *expected);
    }
}

#[test]
fn write_to_slice_dont_require_extra_capacity() {
    let mut buf = [0_u8; 128];

    for (base, pairs) in TEST_CASES {
        let base = IriAbsoluteStr::new(base).expect("should be valid base IRI");
        let resolver = FixedBaseResolver::new(base);

        for (target, expected, _normalized_expected) in *pairs {
            if expected.is_empty() {
                continue;
            }

            let target = IriReferenceStr::new(target).expect("should be valid IRI reference");
            let resolved = resolver.resolve(target);

            let result_small = write_to_slice(&mut buf[..expected.len() - 1], &resolved);
            assert!(result_small.is_err(), "should fail due to too small buffer");

            let result_enough = write_to_slice(&mut buf[..expected.len()], &resolved);
            assert!(result_enough.is_ok(), "buffer should have enough size");
            assert_eq!(
                result_enough.unwrap(),
                *expected,
                "correct result should be written"
            );
        }
    }
}

#[test]
fn resolution_result_live_longer_than_fixed_base_resolver() {
    let mut buf = [0_u8; 128];

    let base = IriAbsoluteStr::new("http://example.com/").expect("should be valid base IRI");
    let reference = IriReferenceStr::new("foo/bar").expect("should be valid IRI reference");

    let resolved = {
        let resolver = FixedBaseResolver::new(base);
        resolver.resolve(reference)
    };
    // Note that `the result of `resolver.resolve()` is still alive here.
    let result = write_to_slice(&mut buf, &resolved).expect("`buf` should have enough capacity");
    assert_eq!(result, "http://example.com/foo/bar");
}

#[test]
fn uri_resolution_against_self_with_normalization() {
    for case in components::TEST_CASES
        .iter()
        .filter(|case| case.is_uri_class() && case.is_absolute())
    {
        let reference = UriStr::new(case.composed).expect("should be valid URI");
        let resolved_normalized = AsRef::<UriReferenceStr>::as_ref(reference)
            .resolve_against(reference.to_absolute())
            .and_normalize();

        assert_eq_display!(resolved_normalized, case.normalized_uri, "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(
            resolved_normalized.to_string(),
            case.normalized_uri,
            "case={case:#?}"
        );
        #[cfg(feature = "alloc")]
        assert_eq!(
            resolved_normalized.to_dedicated_string(),
            case.normalized_uri,
            "case={case:#?}"
        );

        assert_eq!(
            case.is_rfc3986_normalizable(),
            resolved_normalized.ensure_rfc3986_normalizable().is_ok(),
            "case={case:#?}"
        );
    }
}

#[test]
fn iri_resolution_against_self_with_normalization() {
    for case in components::TEST_CASES
        .iter()
        .filter(|case| case.is_iri_class() && case.is_absolute())
    {
        let reference = IriStr::new(case.composed).expect("should be valid IRI");
        let resolved_normalized = AsRef::<IriReferenceStr>::as_ref(reference)
            .resolve_against(reference.to_absolute())
            .and_normalize();

        assert_eq_display!(resolved_normalized, case.normalized_iri, "case={case:#?}");
        #[cfg(feature = "alloc")]
        assert_eq!(
            resolved_normalized.to_string(),
            case.normalized_iri,
            "case={case:#?}"
        );
        #[cfg(feature = "alloc")]
        assert_eq!(
            resolved_normalized.to_dedicated_string(),
            case.normalized_iri,
            "case={case:#?}"
        );

        assert_eq!(
            case.is_rfc3986_normalizable(),
            resolved_normalized.ensure_rfc3986_normalizable().is_ok(),
            "case={case:#?}"
        );
    }
}
