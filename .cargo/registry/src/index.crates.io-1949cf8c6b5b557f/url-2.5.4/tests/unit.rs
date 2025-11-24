// Copyright 2013-2014 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Unit tests
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[macro_use]
extern crate alloc;

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
#[cfg(feature = "std")]
use std::dbg;
use url::{form_urlencoded, Host, Origin, Url};

/// `std` version of `net`
#[cfg(feature = "std")]
pub(crate) mod net {
    pub use std::net::*;
}
/// `no_std` nightly version of `net`
#[cfg(not(feature = "std"))]
pub(crate) mod net {
    pub use core::net::*;
}

use crate::net::{Ipv4Addr, Ipv6Addr};

#[cfg(feature = "std")]
#[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
use std::path::{Path, PathBuf};

// https://rustwasm.github.io/wasm-bindgen/wasm-bindgen-test/usage.html
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::{wasm_bindgen_test as test, wasm_bindgen_test_configure};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn size() {
    use core::mem::size_of;
    assert_eq!(size_of::<Url>(), size_of::<Option<Url>>());
}

#[test]
fn test_relative() {
    let base: Url = "sc://%C3%B1".parse().unwrap();
    let url = base.join("/resources/testharness.js").unwrap();
    assert_eq!(url.as_str(), "sc://%C3%B1/resources/testharness.js");
}

#[test]
fn test_relative_empty() {
    let base: Url = "sc://%C3%B1".parse().unwrap();
    let url = base.join("").unwrap();
    assert_eq!(url.as_str(), "sc://%C3%B1");
}

#[test]
fn test_strip_trailing_spaces_from_opaque_path() {
    let mut url: Url = "data:space   ?query".parse().unwrap();
    url.set_query(None);
    assert_eq!(url.as_str(), "data:space");

    let mut url: Url = "data:space   #hash".parse().unwrap();
    url.set_fragment(None);
    assert_eq!(url.as_str(), "data:space");
}

#[test]
fn test_set_empty_host() {
    let mut base: Url = "moz://foo:bar@servo/baz".parse().unwrap();
    base.set_username("").unwrap();
    assert_eq!(base.as_str(), "moz://:bar@servo/baz");
    base.set_host(None).unwrap();
    assert_eq!(base.as_str(), "moz:/baz");
    base.set_host(Some("servo")).unwrap();
    assert_eq!(base.as_str(), "moz://servo/baz");

    let mut base: Url = "file://server/share/foo/bar".parse().unwrap();
    base.set_host(None).unwrap();
    assert_eq!(base.as_str(), "file:///share/foo/bar");

    let mut base: Url = "file://server/share/foo/bar".parse().unwrap();
    base.set_host(Some("foo")).unwrap();
    assert_eq!(base.as_str(), "file://foo/share/foo/bar");
}

#[test]
fn test_set_empty_username_and_password() {
    let mut base: Url = "moz://foo:bar@servo/baz".parse().unwrap();
    base.set_username("").unwrap();
    assert_eq!(base.as_str(), "moz://:bar@servo/baz");

    base.set_password(Some("")).unwrap();
    assert_eq!(base.as_str(), "moz://servo/baz");

    base.set_password(None).unwrap();
    assert_eq!(base.as_str(), "moz://servo/baz");
}

#[test]
fn test_set_empty_password() {
    let mut base: Url = "moz://foo:bar@servo/baz".parse().unwrap();

    base.set_password(Some("")).unwrap();
    assert_eq!(base.as_str(), "moz://foo@servo/baz");

    base.set_password(None).unwrap();
    assert_eq!(base.as_str(), "moz://foo@servo/baz");
}

#[test]
fn test_set_empty_hostname() {
    use url::quirks;
    let mut base: Url = "moz://foo@servo/baz".parse().unwrap();
    assert!(
        quirks::set_hostname(&mut base, "").is_err(),
        "setting an empty hostname to a url with a username should fail"
    );
    base = "moz://:pass@servo/baz".parse().unwrap();
    assert!(
        quirks::set_hostname(&mut base, "").is_err(),
        "setting an empty hostname to a url with a password should fail"
    );
    base = "moz://servo/baz".parse().unwrap();
    quirks::set_hostname(&mut base, "").unwrap();
    assert_eq!(base.as_str(), "moz:///baz");
}

#[test]
fn test_set_empty_query() {
    let mut base: Url = "moz://example.com/path?query".parse().unwrap();

    base.set_query(Some(""));
    assert_eq!(base.as_str(), "moz://example.com/path?");

    base.set_query(None);
    assert_eq!(base.as_str(), "moz://example.com/path");
}

#[cfg(feature = "std")]
#[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
macro_rules! assert_from_file_path {
    ($path: expr) => {
        assert_from_file_path!($path, $path)
    };
    ($path: expr, $url_path: expr) => {{
        let url = Url::from_file_path(Path::new($path)).unwrap();
        assert_eq!(url.host(), None);
        assert_eq!(url.path(), $url_path);
        assert_eq!(url.to_file_path(), Ok(PathBuf::from($path)));
    }};
}

#[test]
#[cfg(feature = "std")]
#[cfg(any(unix, windows))]
fn new_file_paths() {
    if cfg!(unix) {
        assert_eq!(Url::from_file_path(Path::new("relative")), Err(()));
        assert_eq!(Url::from_file_path(Path::new("../relative")), Err(()));
    }
    if cfg!(windows) {
        assert_eq!(Url::from_file_path(Path::new("relative")), Err(()));
        assert_eq!(Url::from_file_path(Path::new(r"..\relative")), Err(()));
        assert_eq!(Url::from_file_path(Path::new(r"\drive-relative")), Err(()));
        assert_eq!(Url::from_file_path(Path::new(r"\\ucn\")), Err(()));
    }

    if cfg!(unix) {
        assert_from_file_path!("/foo/bar");
        assert_from_file_path!("/foo/ba\0r", "/foo/ba%00r");
        assert_from_file_path!("/foo/ba%00r", "/foo/ba%2500r");
        assert_from_file_path!("/foo/ba\\r", "/foo/ba%5Cr");
    }
}

#[test]
#[cfg(all(feature = "std", unix))]
fn new_path_bad_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::prelude::*;

    let url = Url::from_file_path(Path::new(OsStr::from_bytes(b"/foo/ba\x80r"))).unwrap();
    let os_str = OsStr::from_bytes(b"/foo/ba\x80r");
    assert_eq!(url.to_file_path(), Ok(PathBuf::from(os_str)));
}

#[test]
#[cfg(all(feature = "std", windows))]
fn new_path_windows_fun() {
    assert_from_file_path!(r"C:\foo\bar", "/C:/foo/bar");
    assert_from_file_path!("C:\\foo\\ba\0r", "/C:/foo/ba%00r");

    // Invalid UTF-8
    assert!(Url::parse("file:///C:/foo/ba%80r")
        .unwrap()
        .to_file_path()
        .is_err());

    // test windows canonicalized path
    let path = PathBuf::from(r"\\?\C:\foo\bar");
    assert!(Url::from_file_path(path).is_ok());

    // Percent-encoded drive letter
    let url = Url::parse("file:///C%3A/foo/bar").unwrap();
    assert_eq!(url.to_file_path(), Ok(PathBuf::from(r"C:\foo\bar")));
}

#[test]
#[cfg(all(
    feature = "std",
    any(unix, windows, target_os = "redox", target_os = "wasi")
))]
#[cfg(any(unix, windows))]
fn new_directory_paths() {
    if cfg!(unix) {
        assert_eq!(Url::from_directory_path(Path::new("relative")), Err(()));
        assert_eq!(Url::from_directory_path(Path::new("../relative")), Err(()));

        let url = Url::from_directory_path(Path::new("/foo/bar")).unwrap();
        assert_eq!(url.host(), None);
        assert_eq!(url.path(), "/foo/bar/");
    }
    if cfg!(windows) {
        assert_eq!(Url::from_directory_path(Path::new("relative")), Err(()));
        assert_eq!(Url::from_directory_path(Path::new(r"..\relative")), Err(()));
        assert_eq!(
            Url::from_directory_path(Path::new(r"\drive-relative")),
            Err(())
        );
        assert_eq!(Url::from_directory_path(Path::new(r"\\ucn\")), Err(()));

        let url = Url::from_directory_path(Path::new(r"C:\foo\bar")).unwrap();
        assert_eq!(url.host(), None);
        assert_eq!(url.path(), "/C:/foo/bar/");
    }
}

#[test]
fn path_backslash_fun() {
    let mut special_url = "http://foobar.com".parse::<Url>().unwrap();
    special_url.path_segments_mut().unwrap().push("foo\\bar");
    assert_eq!(special_url.as_str(), "http://foobar.com/foo%5Cbar");

    let mut nonspecial_url = "thing://foobar.com".parse::<Url>().unwrap();
    nonspecial_url.path_segments_mut().unwrap().push("foo\\bar");
    assert_eq!(nonspecial_url.as_str(), "thing://foobar.com/foo\\bar");
}

#[test]
fn from_str() {
    assert!("http://testing.com/this".parse::<Url>().is_ok());
}

#[test]
fn parse_with_params() {
    let url = Url::parse_with_params(
        "http://testing.com/this?dont=clobberme",
        &[("lang", "rust")],
    )
    .unwrap();

    assert_eq!(
        url.as_str(),
        "http://testing.com/this?dont=clobberme&lang=rust"
    );
}

#[test]
fn issue_124() {
    let url: Url = "file:a".parse().unwrap();
    assert_eq!(url.path(), "/a");
    let url: Url = "file:...".parse().unwrap();
    assert_eq!(url.path(), "/...");
    let url: Url = "file:..".parse().unwrap();
    assert_eq!(url.path(), "/");
}

#[test]
#[cfg(feature = "std")]
fn test_equality() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn check_eq(a: &Url, b: &Url) {
        assert_eq!(a, b);

        let mut h1 = DefaultHasher::new();
        a.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        b.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    fn url(s: &str) -> Url {
        let rv = s.parse().unwrap();
        check_eq(&rv, &rv);
        rv
    }

    // Doesn't care if default port is given.
    let a: Url = url("https://example.com/");
    let b: Url = url("https://example.com:443/");
    check_eq(&a, &b);

    // Different ports
    let a: Url = url("http://example.com/");
    let b: Url = url("http://example.com:8080/");
    assert!(a != b, "{:?} != {:?}", a, b);

    // Different scheme
    let a: Url = url("http://example.com/");
    let b: Url = url("https://example.com/");
    assert_ne!(a, b);

    // Different host
    let a: Url = url("http://foo.com/");
    let b: Url = url("http://bar.com/");
    assert_ne!(a, b);

    // Missing path, automatically substituted. Semantically the same.
    let a: Url = url("http://foo.com");
    let b: Url = url("http://foo.com/");
    check_eq(&a, &b);
}

#[test]
fn host() {
    fn assert_host(input: &str, host: Host<&str>) {
        assert_eq!(Url::parse(input).unwrap().host(), Some(host));
    }
    assert_host("http://www.mozilla.org", Host::Domain("www.mozilla.org"));
    assert_host(
        "http://1.35.33.49",
        Host::Ipv4(Ipv4Addr::new(1, 35, 33, 49)),
    );
    assert_host(
        "http://[2001:0db8:85a3:08d3:1319:8a2e:0370:7344]",
        Host::Ipv6(Ipv6Addr::new(
            0x2001, 0x0db8, 0x85a3, 0x08d3, 0x1319, 0x8a2e, 0x0370, 0x7344,
        )),
    );
    assert_host(
        "http://[::]",
        Host::Ipv6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
    );
    assert_host(
        "http://[::1]",
        Host::Ipv6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
    );
    assert_host(
        "http://0x1.0X23.0x21.061",
        Host::Ipv4(Ipv4Addr::new(1, 35, 33, 49)),
    );
    assert_host("http://0x1232131", Host::Ipv4(Ipv4Addr::new(1, 35, 33, 49)));
    assert_host("http://111", Host::Ipv4(Ipv4Addr::new(0, 0, 0, 111)));
    assert!(Url::parse("http://1.35.+33.49").is_err());
    assert!(Url::parse("http://2..2.3").is_err());
    assert!(Url::parse("http://42.0x1232131").is_err());
    assert!(Url::parse("http://192.168.0.257").is_err());

    assert_eq!(Host::Domain("foo"), Host::Domain("foo").to_owned());
    assert_ne!(Host::Domain("foo"), Host::Domain("bar").to_owned());
}

#[test]
fn host_serialization() {
    // libstd’s `Display for Ipv6Addr` serializes 0:0:0:0:0:0:_:_ and 0:0:0:0:0:ffff:_:_
    // using IPv4-like syntax, as suggested in https://tools.ietf.org/html/rfc5952#section-4
    // but https://url.spec.whatwg.org/#concept-ipv6-serializer specifies not to.

    // Not [::0.0.0.2] / [::ffff:0.0.0.2]
    assert_eq!(
        Url::parse("http://[0::2]").unwrap().host_str(),
        Some("[::2]")
    );
    assert_eq!(
        Url::parse("http://[0::ffff:0:2]").unwrap().host_str(),
        Some("[::ffff:0:2]")
    );
}

#[test]
fn test_idna() {
    assert!("http://goșu.ro".parse::<Url>().is_ok());
    assert_eq!(
        Url::parse("http://☃.net/").unwrap().host(),
        Some(Host::Domain("xn--n3h.net"))
    );
    assert!("https://r2---sn-huoa-cvhl.googlevideo.com/crossdomain.xml"
        .parse::<Url>()
        .is_ok());
}

#[test]
fn test_serialization() {
    let data = [
        ("http://example.com/", "http://example.com/"),
        ("http://addslash.com", "http://addslash.com/"),
        ("http://@emptyuser.com/", "http://emptyuser.com/"),
        ("http://:@emptypass.com/", "http://emptypass.com/"),
        ("http://user@user.com/", "http://user@user.com/"),
        (
            "http://user:pass@userpass.com/",
            "http://user:pass@userpass.com/",
        ),
        (
            "http://slashquery.com/path/?q=something",
            "http://slashquery.com/path/?q=something",
        ),
        (
            "http://noslashquery.com/path?q=something",
            "http://noslashquery.com/path?q=something",
        ),
    ];
    for &(input, result) in &data {
        let url = Url::parse(input).unwrap();
        assert_eq!(url.as_str(), result);
    }
}

#[test]
fn test_form_urlencoded() {
    let pairs: &[(Cow<'_, str>, Cow<'_, str>)] = &[
        ("foo".into(), "é&".into()),
        ("bar".into(), "".into()),
        ("foo".into(), "#".into()),
    ];
    let encoded = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish();
    assert_eq!(encoded, "foo=%C3%A9%26&bar=&foo=%23");
    assert_eq!(
        form_urlencoded::parse(encoded.as_bytes()).collect::<Vec<_>>(),
        pairs.to_vec()
    );
}

#[test]
fn test_form_serialize() {
    let encoded = form_urlencoded::Serializer::new(String::new())
        .append_pair("foo", "é&")
        .append_pair("bar", "")
        .append_pair("foo", "#")
        .append_key_only("json")
        .finish();
    assert_eq!(encoded, "foo=%C3%A9%26&bar=&foo=%23&json");
}

#[test]
fn form_urlencoded_encoding_override() {
    let encoded = form_urlencoded::Serializer::new(String::new())
        .encoding_override(Some(&|s| s.as_bytes().to_ascii_uppercase().into()))
        .append_pair("foo", "bar")
        .append_key_only("xml")
        .finish();
    assert_eq!(encoded, "FOO=BAR&XML");
}

#[test]
/// https://github.com/servo/rust-url/issues/61
fn issue_61() {
    let mut url = Url::parse("http://mozilla.org").unwrap();
    url.set_scheme("https").unwrap();
    assert_eq!(url.port(), None);
    assert_eq!(url.port_or_known_default(), Some(443));
    url.check_invariants().unwrap();
}

#[test]
#[cfg(feature = "std")]
#[cfg(any(unix, target_os = "redox", target_os = "wasi"))]
#[cfg(not(windows))]
/// https://github.com/servo/rust-url/issues/197
fn issue_197() {
    let mut url = Url::from_file_path("/").expect("Failed to parse path");
    url.check_invariants().unwrap();
    assert_eq!(
        url,
        Url::parse("file:///").expect("Failed to parse path + protocol")
    );
    url.path_segments_mut()
        .expect("path_segments_mut")
        .pop_if_empty();
}

#[test]
fn issue_241() {
    Url::parse("mailto:").unwrap().cannot_be_a_base();
}

#[test]
/// https://github.com/servo/rust-url/issues/222
fn append_trailing_slash() {
    let mut url: Url = "http://localhost:6767/foo/bar?a=b".parse().unwrap();
    url.check_invariants().unwrap();
    url.path_segments_mut().unwrap().push("");
    url.check_invariants().unwrap();
    assert_eq!(url.to_string(), "http://localhost:6767/foo/bar/?a=b");
}

#[test]
/// https://github.com/servo/rust-url/issues/227
fn extend_query_pairs_then_mutate() {
    let mut url: Url = "http://localhost:6767/foo/bar".parse().unwrap();
    url.query_pairs_mut()
        .extend_pairs(vec![("auth", "my-token")]);
    url.check_invariants().unwrap();
    assert_eq!(
        url.to_string(),
        "http://localhost:6767/foo/bar?auth=my-token"
    );
    url.path_segments_mut().unwrap().push("some_other_path");
    url.check_invariants().unwrap();
    assert_eq!(
        url.to_string(),
        "http://localhost:6767/foo/bar/some_other_path?auth=my-token"
    );
}

#[test]
/// https://github.com/servo/rust-url/issues/222
fn append_empty_segment_then_mutate() {
    let mut url: Url = "http://localhost:6767/foo/bar?a=b".parse().unwrap();
    url.check_invariants().unwrap();
    url.path_segments_mut().unwrap().push("").pop();
    url.check_invariants().unwrap();
    assert_eq!(url.to_string(), "http://localhost:6767/foo/bar?a=b");
}

#[test]
/// https://github.com/servo/rust-url/issues/243
fn test_set_host() {
    let mut url = Url::parse("https://example.net/hello").unwrap();
    url.set_host(Some("foo.com")).unwrap();
    assert_eq!(url.as_str(), "https://foo.com/hello");
    assert!(url.set_host(None).is_err());
    assert_eq!(url.as_str(), "https://foo.com/hello");
    assert!(url.set_host(Some("")).is_err());
    assert_eq!(url.as_str(), "https://foo.com/hello");

    let mut url = Url::parse("foobar://example.net/hello").unwrap();
    url.set_host(None).unwrap();
    assert_eq!(url.as_str(), "foobar:/hello");

    let mut url = Url::parse("foo://ș").unwrap();
    assert_eq!(url.as_str(), "foo://%C8%99");
    url.set_host(Some("goșu.ro")).unwrap();
    assert_eq!(url.as_str(), "foo://go%C8%99u.ro");
}

#[test]
// https://github.com/servo/rust-url/issues/166
fn test_leading_dots() {
    assert_eq!(
        Host::parse(".org").unwrap(),
        Host::Domain(".org".to_owned())
    );
    assert_eq!(Url::parse("file://./foo").unwrap().domain(), Some("."));
}

#[test]
#[cfg(feature = "std")]
/// https://github.com/servo/rust-url/issues/302
fn test_origin_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    let origin = &Url::parse("http://example.net/").unwrap().origin();

    let origins_to_compare = [
        Url::parse("http://example.net:80/").unwrap().origin(),
        Url::parse("http://example.net:81/").unwrap().origin(),
        Url::parse("http://example.net").unwrap().origin(),
        Url::parse("http://example.net/hello").unwrap().origin(),
        Url::parse("https://example.net").unwrap().origin(),
        Url::parse("ftp://example.net").unwrap().origin(),
        Url::parse("file://example.net").unwrap().origin(),
        Url::parse("http://user@example.net/").unwrap().origin(),
        Url::parse("http://user:pass@example.net/")
            .unwrap()
            .origin(),
    ];

    for origin_to_compare in &origins_to_compare {
        if origin == origin_to_compare {
            assert_eq!(hash(origin), hash(origin_to_compare));
        } else {
            assert_ne!(hash(origin), hash(origin_to_compare));
        }
    }

    let opaque_origin = Url::parse("file://example.net").unwrap().origin();
    let same_opaque_origin = Url::parse("file://example.net").unwrap().origin();
    let other_opaque_origin = Url::parse("file://other").unwrap().origin();

    assert_ne!(hash(&opaque_origin), hash(&same_opaque_origin));
    assert_ne!(hash(&opaque_origin), hash(&other_opaque_origin));
}

#[test]
fn test_origin_blob_equality() {
    let origin = &Url::parse("http://example.net/").unwrap().origin();
    let blob_origin = &Url::parse("blob:http://example.net/").unwrap().origin();

    assert_eq!(origin, blob_origin);
}

#[test]
fn test_origin_opaque() {
    assert!(!Origin::new_opaque().is_tuple());
    assert!(!&Url::parse("blob:malformed//").unwrap().origin().is_tuple())
}

#[test]
fn test_origin_unicode_serialization() {
    let data = [
        ("http://😅.com", "http://😅.com"),
        ("ftp://😅:🙂@🙂.com", "ftp://🙂.com"),
        ("https://user@😅.com", "https://😅.com"),
        ("http://😅.🙂:40", "http://😅.🙂:40"),
    ];
    for &(unicode_url, expected_serialization) in &data {
        let origin = Url::parse(unicode_url).unwrap().origin();
        assert_eq!(origin.unicode_serialization(), *expected_serialization);
    }

    let ascii_origins = [
        Url::parse("http://example.net/").unwrap().origin(),
        Url::parse("http://example.net:80/").unwrap().origin(),
        Url::parse("http://example.net:81/").unwrap().origin(),
        Url::parse("http://example.net").unwrap().origin(),
        Url::parse("http://example.net/hello").unwrap().origin(),
        Url::parse("https://example.net").unwrap().origin(),
        Url::parse("ftp://example.net").unwrap().origin(),
        Url::parse("file://example.net").unwrap().origin(),
        Url::parse("http://user@example.net/").unwrap().origin(),
        Url::parse("http://user:pass@example.net/")
            .unwrap()
            .origin(),
        Url::parse("http://127.0.0.1").unwrap().origin(),
    ];
    for ascii_origin in &ascii_origins {
        assert_eq!(
            ascii_origin.ascii_serialization(),
            ascii_origin.unicode_serialization()
        );
    }
}

#[test]
#[cfg(feature = "std")]
#[cfg(any(unix, windows, target_os = "redox", target_os = "wasi"))]
fn test_socket_addrs() {
    use std::net::ToSocketAddrs;

    let data = [
        ("https://127.0.0.1/", "127.0.0.1", 443),
        ("https://127.0.0.1:9742/", "127.0.0.1", 9742),
        ("custom-protocol://127.0.0.1:9742/", "127.0.0.1", 9742),
        ("custom-protocol://127.0.0.1/", "127.0.0.1", 9743),
        ("https://[::1]/", "::1", 443),
        ("https://[::1]:9742/", "::1", 9742),
        ("custom-protocol://[::1]:9742/", "::1", 9742),
        ("custom-protocol://[::1]/", "::1", 9743),
        ("https://localhost/", "localhost", 443),
        ("https://localhost:9742/", "localhost", 9742),
        ("custom-protocol://localhost:9742/", "localhost", 9742),
        ("custom-protocol://localhost/", "localhost", 9743),
    ];

    for (url_string, host, port) in &data {
        let url = url::Url::parse(url_string).unwrap();
        let addrs = url
            .socket_addrs(|| match url.scheme() {
                "custom-protocol" => Some(9743),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            Some(addrs[0]),
            (*host, *port).to_socket_addrs().unwrap().next()
        );
    }
}

#[test]
fn test_no_base_url() {
    let mut no_base_url = Url::parse("mailto:test@example.net").unwrap();

    assert!(no_base_url.cannot_be_a_base());
    assert!(no_base_url.path_segments().is_none());
    assert!(no_base_url.path_segments_mut().is_err());
    assert!(no_base_url.set_host(Some("foo")).is_err());
    assert!(no_base_url
        .set_ip_host("127.0.0.1".parse().unwrap())
        .is_err());

    no_base_url.set_path("/foo");
    assert_eq!(no_base_url.path(), "%2Ffoo");
}

#[test]
fn test_domain() {
    let url = Url::parse("https://127.0.0.1/").unwrap();
    assert_eq!(url.domain(), None);

    let url = Url::parse("mailto:test@example.net").unwrap();
    assert_eq!(url.domain(), None);

    let url = Url::parse("https://example.com/").unwrap();
    assert_eq!(url.domain(), Some("example.com"));
}

#[test]
fn test_query() {
    let url = Url::parse("https://example.com/products?page=2#fragment").unwrap();
    assert_eq!(url.query(), Some("page=2"));
    assert_eq!(
        url.query_pairs().next(),
        Some((Cow::Borrowed("page"), Cow::Borrowed("2")))
    );

    let url = Url::parse("https://example.com/products").unwrap();
    assert!(url.query().is_none());
    assert_eq!(url.query_pairs().count(), 0);

    let url = Url::parse("https://example.com/?country=español").unwrap();
    assert_eq!(url.query(), Some("country=espa%C3%B1ol"));
    assert_eq!(
        url.query_pairs().next(),
        Some((Cow::Borrowed("country"), Cow::Borrowed("español")))
    );

    let url = Url::parse("https://example.com/products?page=2&sort=desc").unwrap();
    assert_eq!(url.query(), Some("page=2&sort=desc"));
    let mut pairs = url.query_pairs();
    assert_eq!(pairs.count(), 2);
    assert_eq!(
        pairs.next(),
        Some((Cow::Borrowed("page"), Cow::Borrowed("2")))
    );
    assert_eq!(
        pairs.next(),
        Some((Cow::Borrowed("sort"), Cow::Borrowed("desc")))
    );
}

#[test]
fn test_fragment() {
    let url = Url::parse("https://example.com/#fragment").unwrap();
    assert_eq!(url.fragment(), Some("fragment"));

    let url = Url::parse("https://example.com/").unwrap();
    assert_eq!(url.fragment(), None);
}

#[test]
fn test_set_ip_host() {
    let mut url = Url::parse("http://example.com").unwrap();

    url.set_ip_host("127.0.0.1".parse().unwrap()).unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));

    url.set_ip_host("::1".parse().unwrap()).unwrap();
    assert_eq!(url.host_str(), Some("[::1]"));
}

#[test]
fn test_set_href() {
    use url::quirks::set_href;

    let mut url = Url::parse("https://existing.url").unwrap();

    assert!(set_href(&mut url, "mal//formed").is_err());

    assert!(set_href(
        &mut url,
        "https://user:pass@domain.com:9742/path/file.ext?key=val&key2=val2#fragment"
    )
    .is_ok());
    assert_eq!(
        url,
        Url::parse("https://user:pass@domain.com:9742/path/file.ext?key=val&key2=val2#fragment")
            .unwrap()
    );
}

#[test]
fn test_domain_encoding_quirks() {
    use url::quirks::{domain_to_ascii, domain_to_unicode};

    let data = [
        ("http://example.com", "", ""),
        ("😅.🙂", "xn--j28h.xn--938h", "😅.🙂"),
        ("example.com", "example.com", "example.com"),
        ("mailto:test@example.net", "", ""),
    ];

    for url in &data {
        assert_eq!(domain_to_ascii(url.0), url.1);
        assert_eq!(domain_to_unicode(url.0), url.2);
    }
}

#[cfg(feature = "expose_internals")]
#[test]
fn test_expose_internals() {
    use url::quirks::internal_components;
    use url::quirks::InternalComponents;

    let url = Url::parse("https://example.com/path/file.ext?key=val&key2=val2#fragment").unwrap();
    let InternalComponents {
        scheme_end,
        username_end,
        host_start,
        host_end,
        port,
        path_start,
        query_start,
        fragment_start,
    } = internal_components(&url);

    assert_eq!(scheme_end, 5);
    assert_eq!(username_end, 8);
    assert_eq!(host_start, 8);
    assert_eq!(host_end, 19);
    assert_eq!(port, None);
    assert_eq!(path_start, 19);
    assert_eq!(query_start, Some(33));
    assert_eq!(fragment_start, Some(51));
}

#[test]
#[cfg(feature = "std")]
#[cfg(windows)]
fn test_windows_unc_path() {
    let url = Url::from_file_path(Path::new(r"\\host\share\path\file.txt")).unwrap();
    assert_eq!(url.as_str(), "file://host/share/path/file.txt");

    let url = Url::from_file_path(Path::new(r"\\höst\share\path\file.txt")).unwrap();
    assert_eq!(url.as_str(), "file://xn--hst-sna/share/path/file.txt");

    let url = Url::from_file_path(Path::new(r"\\192.168.0.1\share\path\file.txt")).unwrap();
    assert_eq!(url.host(), Some(Host::Ipv4(Ipv4Addr::new(192, 168, 0, 1))));

    let path = url.to_file_path().unwrap();
    assert_eq!(path.to_str(), Some(r"\\192.168.0.1\share\path\file.txt"));

    // Another way to write these:
    let url = Url::from_file_path(Path::new(r"\\?\UNC\host\share\path\file.txt")).unwrap();
    assert_eq!(url.as_str(), "file://host/share/path/file.txt");

    // Paths starting with "\\.\" (Local Device Paths) are intentionally not supported.
    let url = Url::from_file_path(Path::new(r"\\.\some\path\file.txt"));
    assert!(url.is_err());
}

#[test]
fn test_syntax_violation_callback() {
    use url::SyntaxViolation::*;
    let violation = Cell::new(None);
    let url = Url::options()
        .syntax_violation_callback(Some(&|v| violation.set(Some(v))))
        .parse("http:////mozilla.org:42")
        .unwrap();
    assert_eq!(url.port(), Some(42));

    let v = violation.take().unwrap();
    assert_eq!(v, ExpectedDoubleSlash);
    assert_eq!(v.description(), "expected //");
    assert_eq!(v.to_string(), "expected //");
}

#[test]
fn test_syntax_violation_callback_lifetimes() {
    use url::SyntaxViolation::*;
    let violation = Cell::new(None);
    let vfn = |s| violation.set(Some(s));

    let url = Url::options()
        .syntax_violation_callback(Some(&vfn))
        .parse("http:////mozilla.org:42")
        .unwrap();
    assert_eq!(url.port(), Some(42));
    assert_eq!(violation.take(), Some(ExpectedDoubleSlash));

    let url = Url::options()
        .syntax_violation_callback(Some(&vfn))
        .parse("http://mozilla.org\\path")
        .unwrap();
    assert_eq!(url.path(), "/path");
    assert_eq!(violation.take(), Some(Backslash));
}

#[test]
fn test_syntax_violation_callback_types() {
    use url::SyntaxViolation::*;

    let data = [
        ("http://mozilla.org/\\foo", Backslash, "backslash"),
        (" http://mozilla.org", C0SpaceIgnored, "leading or trailing control or space character are ignored in URLs"),
        ("http://user:pass@mozilla.org", EmbeddedCredentials, "embedding authentication information (username or password) in an URL is not recommended"),
        ("http:///mozilla.org", ExpectedDoubleSlash, "expected //"),
        ("file:/foo.txt", ExpectedFileDoubleSlash, "expected // after file:"),
        ("file://mozilla.org/c:/file.txt", FileWithHostAndWindowsDrive, "file: with host and Windows drive letter"),
        ("http://mozilla.org/^", NonUrlCodePoint, "non-URL code point"),
        ("http://mozilla.org/#\x000", NullInFragment, "NULL characters are ignored in URL fragment identifiers"),
        ("http://mozilla.org/%1", PercentDecode, "expected 2 hex digits after %"),
        ("http://mozilla.org\t/foo", TabOrNewlineIgnored, "tabs or newlines are ignored in URLs"),
        ("http://user@:pass@mozilla.org", UnencodedAtSign, "unencoded @ sign in username or password")
    ];

    for test_case in &data {
        let violation = Cell::new(None);
        Url::options()
            .syntax_violation_callback(Some(&|v| violation.set(Some(v))))
            .parse(test_case.0)
            .unwrap();

        let v = violation.take();
        assert_eq!(v, Some(test_case.1));
        assert_eq!(v.unwrap().description(), test_case.2);
        assert_eq!(v.unwrap().to_string(), test_case.2);
    }
}

#[test]
fn test_options_reuse() {
    use url::SyntaxViolation::*;
    let violations = RefCell::new(Vec::new());
    let vfn = |v| violations.borrow_mut().push(v);

    let options = Url::options().syntax_violation_callback(Some(&vfn));
    let url = options.parse("http:////mozilla.org").unwrap();

    let options = options.base_url(Some(&url));
    let url = options.parse("/sub\\path").unwrap();
    assert_eq!(url.as_str(), "http://mozilla.org/sub/path");
    assert_eq!(*violations.borrow(), vec!(ExpectedDoubleSlash, Backslash));
}

/// https://github.com/servo/rust-url/issues/505
#[test]
#[cfg(all(feature = "std", windows))]
fn test_url_from_file_path() {
    use std::path::PathBuf;
    use url::Url;

    let p = PathBuf::from("c:///");
    let u = Url::from_file_path(p).unwrap();
    let path = u.to_file_path().unwrap();
    assert_eq!("C:\\", path.to_str().unwrap());
}

/// https://github.com/servo/rust-url/issues/505
#[cfg(feature = "std")]
#[cfg(any(unix, target_os = "redox", target_os = "wasi"))]
#[cfg(not(windows))]
#[test]
fn test_url_from_file_path() {
    use std::path::PathBuf;
    use url::Url;

    let p = PathBuf::from("/c:/");
    let u = Url::from_file_path(p).unwrap();
    let path = u.to_file_path().unwrap();
    assert_eq!("/c:/", path.to_str().unwrap());
}

#[test]
fn test_non_special_path() {
    let mut db_url = url::Url::parse("postgres://postgres@localhost/").unwrap();
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/");
    db_url.set_path("diesel_foo");
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/diesel_foo");
    assert_eq!(db_url.path(), "/diesel_foo");
}

#[test]
fn test_non_special_path2() {
    let mut db_url = url::Url::parse("postgres://postgres@localhost/").unwrap();
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/");
    db_url.set_path("");
    assert_eq!(db_url.path(), "");
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost");
    db_url.set_path("foo");
    assert_eq!(db_url.path(), "/foo");
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/foo");
    db_url.set_path("/bar");
    assert_eq!(db_url.path(), "/bar");
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/bar");
}

#[test]
fn test_non_special_path3() {
    let mut db_url = url::Url::parse("postgres://postgres@localhost/").unwrap();
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/");
    db_url.set_path("/");
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/");
    assert_eq!(db_url.path(), "/");
    db_url.set_path("/foo");
    assert_eq!(db_url.as_str(), "postgres://postgres@localhost/foo");
    assert_eq!(db_url.path(), "/foo");
}

#[test]
fn test_set_scheme_to_file_with_host() {
    let mut url: Url = "http://localhost:6767/foo/bar".parse().unwrap();
    let result = url.set_scheme("file");
    assert_eq!(url.to_string(), "http://localhost:6767/foo/bar");
    assert_eq!(result, Err(()));
}

#[test]
fn no_panic() {
    let mut url = Url::parse("arhttpsps:/.//eom/dae.com/\\\\t\\:").unwrap();
    url::quirks::set_hostname(&mut url, "//eom/datcom/\\\\t\\://eom/data.cs").unwrap();
}

#[test]
fn test_null_host_with_leading_empty_path_segment() {
    // since Note in item 3 of URL serializing in the URL Standard
    // https://url.spec.whatwg.org/#url-serializing
    let url = Url::parse("m:/.//\\").unwrap();
    let encoded = url.as_str();
    let reparsed = Url::parse(encoded).unwrap();
    assert_eq!(reparsed, url);
}

#[test]
fn pop_if_empty_in_bounds() {
    let mut url = Url::parse("m://").unwrap();
    let mut segments = url.path_segments_mut().unwrap();
    segments.pop_if_empty();
    segments.pop();
}

#[test]
fn test_slicing() {
    use url::Position::*;

    #[derive(Default)]
    struct ExpectedSlices<'a> {
        full: &'a str,
        scheme: &'a str,
        username: &'a str,
        password: &'a str,
        host: &'a str,
        port: &'a str,
        path: &'a str,
        query: &'a str,
        fragment: &'a str,
    }

    let data = [
        ExpectedSlices {
            full: "https://user:pass@domain.com:9742/path/file.ext?key=val&key2=val2#fragment",
            scheme: "https",
            username: "user",
            password: "pass",
            host: "domain.com",
            port: "9742",
            path: "/path/file.ext",
            query: "key=val&key2=val2",
            fragment: "fragment",
        },
        ExpectedSlices {
            full: "https://domain.com:9742/path/file.ext#fragment",
            scheme: "https",
            host: "domain.com",
            port: "9742",
            path: "/path/file.ext",
            fragment: "fragment",
            ..Default::default()
        },
        ExpectedSlices {
            full: "https://domain.com:9742/path/file.ext",
            scheme: "https",
            host: "domain.com",
            port: "9742",
            path: "/path/file.ext",
            ..Default::default()
        },
        ExpectedSlices {
            full: "blob:blob-info",
            scheme: "blob",
            path: "blob-info",
            ..Default::default()
        },
    ];

    for expected_slices in &data {
        let url = Url::parse(expected_slices.full).unwrap();
        assert_eq!(&url[..], expected_slices.full);
        assert_eq!(&url[BeforeScheme..AfterScheme], expected_slices.scheme);
        assert_eq!(
            &url[BeforeUsername..AfterUsername],
            expected_slices.username
        );
        assert_eq!(
            &url[BeforePassword..AfterPassword],
            expected_slices.password
        );
        assert_eq!(&url[BeforeHost..AfterHost], expected_slices.host);
        assert_eq!(&url[BeforePort..AfterPort], expected_slices.port);
        assert_eq!(&url[BeforePath..AfterPath], expected_slices.path);
        assert_eq!(&url[BeforeQuery..AfterQuery], expected_slices.query);
        assert_eq!(
            &url[BeforeFragment..AfterFragment],
            expected_slices.fragment
        );
        assert_eq!(&url[..AfterFragment], expected_slices.full);
    }
}

#[test]
fn test_make_relative() {
    let tests = [
        (
            "http://127.0.0.1:8080/test",
            "http://127.0.0.1:8080/test",
            "",
        ),
        (
            "http://127.0.0.1:8080/test",
            "http://127.0.0.1:8080/test/",
            "test/",
        ),
        (
            "http://127.0.0.1:8080/test/",
            "http://127.0.0.1:8080/test",
            "../test",
        ),
        (
            "http://127.0.0.1:8080/",
            "http://127.0.0.1:8080/?foo=bar#123",
            "?foo=bar#123",
        ),
        (
            "http://127.0.0.1:8080/",
            "http://127.0.0.1:8080/test/video",
            "test/video",
        ),
        (
            "http://127.0.0.1:8080/test",
            "http://127.0.0.1:8080/test/video",
            "test/video",
        ),
        (
            "http://127.0.0.1:8080/test/",
            "http://127.0.0.1:8080/test/video",
            "video",
        ),
        (
            "http://127.0.0.1:8080/test",
            "http://127.0.0.1:8080/test2/video",
            "test2/video",
        ),
        (
            "http://127.0.0.1:8080/test/",
            "http://127.0.0.1:8080/test2/video",
            "../test2/video",
        ),
        (
            "http://127.0.0.1:8080/test/bla",
            "http://127.0.0.1:8080/test2/video",
            "../test2/video",
        ),
        (
            "http://127.0.0.1:8080/test/bla/",
            "http://127.0.0.1:8080/test2/video",
            "../../test2/video",
        ),
        (
            "http://127.0.0.1:8080/test/?foo=bar#123",
            "http://127.0.0.1:8080/test/video",
            "video",
        ),
        (
            "http://127.0.0.1:8080/test/",
            "http://127.0.0.1:8080/test/video?baz=meh#456",
            "video?baz=meh#456",
        ),
        (
            "http://127.0.0.1:8080/test",
            "http://127.0.0.1:8080/test?baz=meh#456",
            "?baz=meh#456",
        ),
        (
            "http://127.0.0.1:8080/test/",
            "http://127.0.0.1:8080/test?baz=meh#456",
            "../test?baz=meh#456",
        ),
        (
            "http://127.0.0.1:8080/test/",
            "http://127.0.0.1:8080/test/?baz=meh#456",
            "?baz=meh#456",
        ),
        (
            "http://127.0.0.1:8080/test/?foo=bar#123",
            "http://127.0.0.1:8080/test/video?baz=meh#456",
            "video?baz=meh#456",
        ),
        (
            "http://127.0.0.1:8080/file.txt",
            "http://127.0.0.1:8080/test/file.txt",
            "test/file.txt",
        ),
        (
            "http://127.0.0.1:8080/not_equal.txt",
            "http://127.0.0.1:8080/test/file.txt",
            "test/file.txt",
        ),
    ];

    for (base, uri, relative) in &tests {
        let base_uri = url::Url::parse(base).unwrap();
        let relative_uri = url::Url::parse(uri).unwrap();
        let make_relative = base_uri.make_relative(&relative_uri).unwrap();
        assert_eq!(
            make_relative, *relative,
            "base: {}, uri: {}, relative: {}",
            base, uri, relative
        );
        assert_eq!(
            base_uri.join(relative).unwrap().as_str(),
            *uri,
            "base: {}, uri: {}, relative: {}",
            base,
            uri,
            relative
        );
    }

    let error_tests = [
        ("http://127.0.0.1:8080/", "https://127.0.0.1:8080/test/"),
        ("http://127.0.0.1:8080/", "http://127.0.0.1:8081/test/"),
        ("http://127.0.0.1:8080/", "http://127.0.0.2:8080/test/"),
        ("mailto:a@example.com", "mailto:b@example.com"),
    ];

    for (base, uri) in &error_tests {
        let base_uri = url::Url::parse(base).unwrap();
        let relative_uri = url::Url::parse(uri).unwrap();
        let make_relative = base_uri.make_relative(&relative_uri);
        assert_eq!(make_relative, None, "base: {}, uri: {}", base, uri);
    }
}

#[test]
fn test_has_authority() {
    let url = Url::parse("mailto:joe@example.com").unwrap();
    assert!(!url.has_authority());
    let url = Url::parse("unix:/run/foo.socket").unwrap();
    assert!(!url.has_authority());
    let url = Url::parse("file:///tmp/foo").unwrap();
    assert!(url.has_authority());
    let url = Url::parse("http://example.com/tmp/foo").unwrap();
    assert!(url.has_authority());
}

#[test]
fn test_authority() {
    let url = Url::parse("mailto:joe@example.com").unwrap();
    assert_eq!(url.authority(), "");
    let url = Url::parse("unix:/run/foo.socket").unwrap();
    assert_eq!(url.authority(), "");
    let url = Url::parse("file:///tmp/foo").unwrap();
    assert_eq!(url.authority(), "");
    let url = Url::parse("http://example.com/tmp/foo").unwrap();
    assert_eq!(url.authority(), "example.com");
    let url = Url::parse("ftp://127.0.0.1:21/").unwrap();
    assert_eq!(url.authority(), "127.0.0.1");
    let url = Url::parse("ftp://user@127.0.0.1:2121/").unwrap();
    assert_eq!(url.authority(), "user@127.0.0.1:2121");
    let url = Url::parse("https://:@example.com/").unwrap();
    assert_eq!(url.authority(), "example.com");
    let url = Url::parse("https://:password@[::1]:8080/").unwrap();
    assert_eq!(url.authority(), ":password@[::1]:8080");
    let url = Url::parse("gopher://user:@àlex.example.com:70").unwrap();
    assert_eq!(url.authority(), "user@%C3%A0lex.example.com:70");
    let url = Url::parse("irc://àlex:àlex@àlex.рф.example.com:6667/foo").unwrap();
    assert_eq!(
        url.authority(),
        "%C3%A0lex:%C3%A0lex@%C3%A0lex.%D1%80%D1%84.example.com:6667"
    );
    let url = Url::parse("https://àlex:àlex@àlex.рф.example.com:443/foo").unwrap();
    assert_eq!(
        url.authority(),
        "%C3%A0lex:%C3%A0lex@xn--lex-8ka.xn--p1ai.example.com"
    );
}

#[test]
/// https://github.com/servo/rust-url/issues/838
fn test_file_with_drive() {
    let s1 = "fIlE:p:?../";
    let url = url::Url::parse(s1).unwrap();
    assert_eq!(url.to_string(), "file:///p:?../");
    assert_eq!(url.path(), "/p:");

    let testcases = [
        ("a", "file:///p:/a"),
        ("", "file:///p:?../"),
        ("?x", "file:///p:?x"),
        (".", "file:///p:/"),
        ("..", "file:///p:/"),
        ("../", "file:///p:/"),
    ];

    for case in &testcases {
        let url2 = url::Url::join(&url, case.0).unwrap();
        assert_eq!(url2.to_string(), case.1);
    }
}

#[test]
/// Similar to test_file_with_drive, but with a path
/// that could be confused for a drive.
fn test_file_with_drive_and_path() {
    let s1 = "fIlE:p:/x|?../";
    let url = url::Url::parse(s1).unwrap();
    assert_eq!(url.to_string(), "file:///p:/x|?../");
    assert_eq!(url.path(), "/p:/x|");
    let s2 = "a";
    let url2 = url::Url::join(&url, s2).unwrap();
    assert_eq!(url2.to_string(), "file:///p:/a");
}

#[cfg(feature = "std")]
#[test]
fn issue_864() {
    let mut url = url::Url::parse("file://").unwrap();
    dbg!(&url);
    url.set_path("x");
    dbg!(&url);
}

#[test]
fn issue_974() {
    let mut url = url::Url::parse("http://example.com:8000").unwrap();
    let _ = url::quirks::set_port(&mut url, "\u{0000}9000");
    assert_eq!(url.port(), Some(8000));
}

#[cfg(feature = "serde")]
#[test]
fn serde_error_message() {
    use serde::Deserialize;
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct TypeWithUrl {
        url: Url,
    }

    let err = serde_json::from_str::<TypeWithUrl>(r#"{"url": "§invalid#+#*Ä"}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        r#"relative URL without a base: "§invalid#+#*Ä" at line 1 column 25"#
    );
}
