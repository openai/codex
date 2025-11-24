//! Tests for builder.

mod components;
#[macro_use]
mod utils;

use iri_string::build::Builder;
use iri_string::format::write_to_slice;
use iri_string::types::*;

use self::components::{Components, TestCase, TEST_CASES};

/// Pairs of components and composed IRI should be consistent.
///
/// This also (implicitly) tests that build-and-decompose and decompose-and-build
/// operations are identity conversions.
#[test]
fn consistent_components_and_composed() {
    for case in TEST_CASES.iter().copied() {
        let mut builder = Builder::new();
        case.components.feed_builder(&mut builder, false);

        // composed -> components.
        let built = builder
            .build::<IriReferenceStr>()
            .expect("should be valid IRI reference");
        assert_eq_display!(built, case.composed);

        // components -> composed.
        let composed = IriReferenceStr::new(case.composed).expect("should be valid IRI reference");
        let scheme = composed.scheme_str();
        let (user, password, host, port) = match composed.authority_components() {
            None => (None, None, None, None),
            Some(authority) => {
                let (user, password) = match authority.userinfo() {
                    None => (None, None),
                    Some(userinfo) => match userinfo.find(':').map(|pos| userinfo.split_at(pos)) {
                        Some((user, password)) => (Some(user), Some(&password[1..])),
                        None => (Some(userinfo), None),
                    },
                };
                (user, password, Some(authority.host()), authority.port())
            }
        };
        let path = composed.path_str();
        let query = composed.query().map(|s| s.as_str());
        let fragment = composed.fragment().map(|s| s.as_str());

        let roundtrip_result = Components {
            scheme,
            user,
            password,
            host,
            port,
            path,
            query,
            fragment,
        };
        assert_eq!(roundtrip_result, case.components, "case={case:#?}");
    }
}

fn assert_builds_for_case(case: &TestCase<'_>, builder: &Builder<'_>) {
    if case.is_iri_class() {
        {
            let built = builder
                .clone()
                .build::<IriReferenceStr>()
                .expect("should be valid IRI reference");
            assert_eq_display!(built, case.composed);
        }
        {
            let built = builder.clone().build::<IriStr>();
            if case.is_absolute() {
                let built = built.expect("should be valid IRI");
                assert_eq_display!(built, case.composed);
            } else {
                assert!(built.is_err(), "should be invalid as IRI");
            }
        }
        {
            let built = builder.clone().build::<IriAbsoluteStr>();
            if case.is_absolute_without_fragment() {
                let built = built.expect("should be valid absolute IRI");
                assert_eq_display!(built, case.composed);
            } else {
                assert!(built.is_err(), "should be invalid as absolute IRI");
            }
        }
        {
            let built = builder.clone().build::<IriRelativeStr>();
            if case.is_relative() {
                let built = built.expect("should be valid relative IRI reference");
                assert_eq_display!(built, case.composed);
            } else {
                assert!(
                    built.is_err(),
                    "should be invalid as relative IRI reference"
                );
            }
        }
    }
    if case.is_uri_class() {
        {
            let built = builder
                .clone()
                .build::<UriReferenceStr>()
                .expect("should be valid URI reference");
            assert_eq_display!(built, case.composed);
        }
        {
            let built = builder.clone().build::<UriStr>();
            if case.is_absolute() {
                let built = built.expect("should be valid URI");
                assert_eq_display!(built, case.composed);
            } else {
                assert!(built.is_err(), "should be invalid as URI");
            }
        }
        {
            let built = builder.clone().build::<UriAbsoluteStr>();
            if case.is_absolute_without_fragment() {
                let built = built.expect("should be valid absolute URI");
                assert_eq_display!(built, case.composed);
            } else {
                assert!(built.is_err(), "should be invalid as absolute URI");
            }
        }
        {
            let built = builder.clone().build::<UriRelativeStr>();
            if case.is_relative() {
                let built = built.expect("should be valid relative URI reference");
                assert_eq_display!(built, case.composed);
            } else {
                assert!(
                    built.is_err(),
                    "should be invalid as relative URI reference"
                );
            }
        }
    }
}

/// Build should succeed or fail, depending on the target syntax and the source string.
#[test]
fn build_simple() {
    for case in TEST_CASES.iter() {
        let mut builder = Builder::new();
        case.components.feed_builder(&mut builder, false);

        assert_builds_for_case(case, &builder);
    }
}

/// Fields of a builder can be unset.
#[test]
fn reuse_dirty_builder() {
    let dirty = {
        let mut b = Builder::new();
        b.scheme("scheme");
        b.userinfo(("user", "password"));
        b.host("host");
        b.port("90127");
        b.path("/path/path-again");
        b.query("query");
        b.fragment("fragment");
        b
    };
    for case in TEST_CASES.iter() {
        let mut builder = dirty.clone();
        case.components.feed_builder(&mut builder, true);

        assert_builds_for_case(case, &builder);
    }
}

/// Builder can normalize absolute IRIs.
#[test]
fn build_normalized_absolute() {
    for case in TEST_CASES.iter().filter(|case| case.is_absolute()) {
        assert!(
            !case.is_relative(),
            "every IRI is absolute or relative, but not both"
        );

        let mut builder = Builder::new();
        case.components.feed_builder(&mut builder, false);
        builder.normalize();

        let built_iri = builder
            .clone()
            .build::<IriStr>()
            .expect("should be valid IRI reference");
        assert_eq_display!(built_iri, case.normalized_iri, "case={case:#?}");

        if case.is_uri_class() {
            let built_uri = builder
                .build::<UriStr>()
                .expect("should be valid URI reference");
            assert_eq_display!(built_uri, case.normalized_uri, "case={case:#?}");
        }
    }
}

/// Builder can normalize relative IRIs.
#[test]
fn build_normalized_relative() {
    for case in TEST_CASES.iter().filter(|case| case.is_relative()) {
        assert!(
            !case.is_absolute(),
            "every IRI is absolute or relative, but not both"
        );

        let mut builder = Builder::new();
        case.components.feed_builder(&mut builder, false);
        builder.normalize();

        let built = builder
            .clone()
            .build::<IriRelativeStr>()
            .expect("should be valid relative IRI reference");
        assert_eq_display!(built, case.normalized_iri, "case={case:#?}");

        if case.is_uri_class() {
            let built_uri = builder
                .build::<UriReferenceStr>()
                .expect("should be valid relative URI reference");
            assert_eq_display!(built_uri, case.normalized_uri, "case={case:#?}");
        }
    }
}

/// Build result can judge RFC3986-normalizedness correctly.
#[test]
fn build_normalizedness() {
    for case in TEST_CASES.iter().filter(|case| case.is_absolute()) {
        let mut builder = Builder::new();
        case.components.feed_builder(&mut builder, false);
        builder.normalize();

        let built = builder
            .clone()
            .build::<IriStr>()
            .expect("should be valid IRI reference");
        let built_judge = built.ensure_rfc3986_normalizable().is_ok();
        assert_eq!(
            built_judge,
            case.is_rfc3986_normalizable(),
            "RFC3986-normalizedness should be correctly judged: case={case:#?}"
        );

        let mut buf = [0_u8; 512];
        let s = write_to_slice(&mut buf, &built).expect("not enough buffer");
        let built_slice = IriStr::new(s).expect("should be valid IRI reference");
        assert!(
            built_slice.is_normalized_but_authorityless_relative_path_preserved(),
            "should be normalized"
        );
        let slice_judge = built_slice.is_normalized_rfc3986();

        assert_eq!(
            slice_judge, built_judge,
            "RFC3986-normalizedness should be consistently judged: case={case:#?}"
        );
    }
}

/// `Builder::port` should accept `u8` value.
#[test]
fn set_port_u8() {
    let mut builder = Builder::new();
    builder.port(8_u8);
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(built, "//:8", "should accept `u8`");
}

/// `Builder::port` should accept `u16` value.
#[test]
fn set_port_u16() {
    let mut builder = Builder::new();
    builder.port(65535_u16);
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(built, "//:65535", "should accept `u16`");
}

/// `Builder::port` should accept `&str` value.
#[test]
fn set_port_str() {
    let mut builder = Builder::new();
    builder.port("8080");
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(built, "//:8080", "should accept `&str`");
}

/// `Builder::port` should accept `&str` value even it is quite large.
#[test]
fn set_port_str_large() {
    let mut builder = Builder::new();
    builder.port("12345678901234567890");
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(
        built,
        "//:12345678901234567890",
        "should accept `&str` even it is quite large"
    );
}

/// `Builder::ip_address` should accept `std::net::Ipv4Addr` value.
#[test]
#[cfg(feature = "std")]
fn set_ip_address_ipv4addr() {
    let mut builder = Builder::new();
    builder.ip_address(std::net::Ipv4Addr::new(192, 0, 2, 0));
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(built, "//192.0.2.0", "should accept `std::net::Ipv4Addr`");
}

/// `Builder::ip_address` should accept `std::net::Ipv6Addr` value.
#[test]
#[cfg(feature = "std")]
fn set_ip_address_ipv6addr() {
    let mut builder = Builder::new();
    builder.ip_address(std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(
        built,
        "//[2001:db8::1]",
        "should accept `std::net::Ipv6Addr`"
    );
}

/// `Builder::ip_address` should accept `std::net::IpAddr` value.
#[test]
#[cfg(feature = "std")]
fn set_ip_address_ipaddr() {
    let mut builder = Builder::new();
    builder.ip_address(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 0)));
    let built = builder
        .clone()
        .build::<UriReferenceStr>()
        .expect("should be valid URI reference");
    assert_eq_display!(built, "//192.0.2.0", "should accept `std::net::IpAddr`");
}

/// `Builder::userinfo` should accept `&str`.
#[test]
fn set_userinfo_str() {
    let mut builder = Builder::new();
    {
        builder.userinfo("user:password");
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(built, "//user:password@", "should accept `&str`");
    }
    {
        builder.userinfo("arbitrary-valid-string");
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(built, "//arbitrary-valid-string@", "should accept `&str`");
    }
    {
        builder.userinfo("arbitrary:valid:string");
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(built, "//arbitrary:valid:string@", "should accept `&str`");
    }
}

/// `Builder::userinfo` should accept `(&str, &str)`.
#[test]
fn set_userinfo_pair_str_str() {
    let mut builder = Builder::new();
    {
        builder.userinfo(("user", "password"));
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(built, "//user:password@", "should accept `&str`");
    }
    {
        builder.userinfo(("", ""));
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(built, "//:@", "empty user and password should be preserved");
    }
}

/// `Builder::userinfo` should accept `(&str, Option<&str>)`.
#[test]
fn set_userinfo_pair_str_optstr() {
    let mut builder = Builder::new();
    {
        builder.userinfo(("user", Some("password")));
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(
            built,
            "//user:password@",
            "should accept `(&str, Option<&str>)`"
        );
    }
    {
        builder.userinfo(("", Some("")));
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(built, "//:@", "empty user and password should be preserved");
    }
    {
        builder.userinfo(("user", None));
        let built = builder
            .clone()
            .build::<UriReferenceStr>()
            .expect("should be valid URI reference");
        assert_eq_display!(
            built,
            "//user@",
            "password given as `None` should be absent"
        );
    }
}

/// Builder should reject a colon in user.
#[test]
fn user_with_colon() {
    let mut builder = Builder::new();
    builder.userinfo(("us:er", Some("password")));
    let result = builder.clone().build::<UriReferenceStr>();
    assert!(result.is_err(), "`user` part cannot have a colon");
}

/// Builder should be able to build a normalized IRI even when it requires
/// edge case handling of RFC 3986 normalization.
#[test]
fn normalize_double_slash_prefix() {
    let mut builder = Builder::new();
    builder.scheme("scheme");
    builder.path("/..//bar");
    builder.normalize();
    let built = builder
        .build::<IriStr>()
        .expect("normalizable by `/.` path prefix");
    // Naive application of RFC 3986 normalization/resolution algorithm
    // results in `scheme://bar`, but this is unintentional. `bar` should be
    // the second path segment, not a host. So this should be rejected.
    assert!(
        built.ensure_rfc3986_normalizable().is_err(),
        "not normalizable by RFC 3986 algorithm"
    );
    // In contrast to RFC 3986, WHATWG URL Standard defines serialization
    // algorithm and handles this case specially. In this case, the result
    // is `scheme:/.//bar`, this won't be considered fully normalized from
    // the RFC 3986 point of view, but more normalization would be
    // impossible and this would practically work in most situations.
    assert_eq_display!(built, "scheme:/.//bar");
}

/// Builder should be able to build a normalized IRI even when it requires
/// edge case handling of RFC 3986 normalization.
#[test]
fn absolute_double_slash_path_without_authority() {
    let mut builder = Builder::new();
    builder.scheme("scheme");
    builder.path("//bar");

    // Should fail without normalization.
    {
        let result = builder.clone().build::<IriStr>();
        assert!(
            result.is_err(),
            "`scheme://bar` is unintended so the build should fail"
        );
    }

    // With normalization, the build succeeds.
    builder.normalize();
    let built = builder
        .build::<IriStr>()
        .expect("normalizable by `/.` path prefix");
    // Naive application of RFC 3986 normalization/resolution algorithm
    // results in `scheme://bar`, but this is unintentional. `bar` should be
    // the second path segment, not a host. So this should be rejected.
    assert!(
        built.ensure_rfc3986_normalizable().is_err(),
        "not normalizable by RFC 3986 algorithm"
    );
    // In contrast to RFC 3986, WHATWG URL Standard defines serialization
    // algorithm and handles this case specially. In this case, the result
    // is `scheme:/.//bar`, this won't be considered fully normalized from
    // the RFC 3986 point of view, but more normalization would be
    // impossible and this would practically work in most situations.
    assert_eq_display!(built, "scheme:/.//bar");
}

/// Authority requires the path to be empty or absolute (without normalization enabled).
#[test]
fn authority_and_relative_path() {
    let mut builder = Builder::new();
    builder.host("example.com");
    builder.path("relative/path");
    assert!(
        builder.clone().build::<IriReferenceStr>().is_err(),
        "authority requires the path to be empty or absolute"
    );

    // Even if normalization is enabled, the relative path is unacceptable.
    builder.normalize();
    assert!(
        builder.build::<IriReferenceStr>().is_err(),
        "authority requires the path to be empty or absolute"
    );
}

#[test]
fn no_authority_and_double_slash_prefix_without_normalization() {
    let mut builder = Builder::new();
    // This would be interpreted as "network-path reference" (see RFC 3986
    // section 4.2), so this should be rejected.
    builder.path("//double-slash");
    assert!(builder.build::<IriReferenceStr>().is_err());
}

#[test]
fn no_authority_and_double_slash_prefix_with_normalization() {
    let mut builder = Builder::new();
    builder.path("//double-slash");
    builder.normalize();
    let built = builder
        .build::<IriReferenceStr>()
        .expect("normalizable by `/.` path prefix");
    assert_eq_display!(built, "/.//double-slash");
    assert!(built.ensure_rfc3986_normalizable().is_err());
}

#[test]
fn no_authority_and_relative_first_segment_colon() {
    let mut builder = Builder::new();
    // This would be interpreted as scheme `foo` and host `bar`,
    // so this should be rejected.
    builder.path("foo:bar");
    assert!(builder.clone().build::<IriReferenceStr>().is_err());

    // Normalization does not change the situation.
    builder.normalize();
    assert!(builder.build::<IriReferenceStr>().is_err());
}
