use std::str::FromStr;

use super::{ErrorKind, InvalidUri, Port, Uri, URI_CHARS};

#[test]
fn test_char_table() {
    for (i, &v) in URI_CHARS.iter().enumerate() {
        if v != 0 {
            assert_eq!(i, v as usize);
        }
    }
}

macro_rules! part {
    ($s:expr) => {
        Some(&$s.parse().unwrap())
    };
}

macro_rules! test_parse {
    (
        $test_name:ident,
        $str:expr,
        $alt:expr,
        $($method:ident = $value:expr,)*
    ) => (
        #[test]
        fn $test_name() {
            let orig_str = $str;
            let uri = match Uri::from_str(orig_str) {
                Ok(uri) => uri,
                Err(err) => {
                    panic!("parse error {:?} from {:?}", err, orig_str);
                },
            };
            $(
            assert_eq!(uri.$method(), $value, "{}: uri = {:?}", stringify!($method), uri);
            )+
            assert_eq!(uri, orig_str, "partial eq to original str");
            assert_eq!(uri, uri.clone(), "clones are equal");

            let new_str = uri.to_string();
            let new_uri = Uri::from_str(&new_str).expect("to_string output parses again as a Uri");
            assert_eq!(new_uri, orig_str, "round trip still equals original str");

            const ALT: &'static [&'static str] = &$alt;

            for &alt in ALT.iter() {
                let other: Uri = alt.parse().unwrap();
                assert_eq!(uri, *alt);
                assert_eq!(uri, other);
            }
        }
    );
}

test_parse! {
    test_uri_parse_path_and_query,
    "/some/path/here?and=then&hello#and-bye",
    [],

    scheme = None,
    authority = None,
    path = "/some/path/here",
    query = Some("and=then&hello"),
    host = None,
}

test_parse! {
    test_uri_parse_absolute_form,
    "http://127.0.0.1:61761/chunks",
    [],

    scheme = part!("http"),
    authority = part!("127.0.0.1:61761"),
    path = "/chunks",
    query = None,
    host = Some("127.0.0.1"),
    port = Port::from_str("61761").ok(),
}

test_parse! {
    test_uri_parse_absolute_form_without_path,
    "https://127.0.0.1:61761",
    ["https://127.0.0.1:61761/"],

    scheme = part!("https"),
    authority = part!("127.0.0.1:61761"),
    path = "/",
    query = None,
    host = Some("127.0.0.1"),
    port = Port::from_str("61761").ok(),
}

test_parse! {
    test_uri_parse_asterisk_form,
    "*",
    [],

    scheme = None,
    authority = None,
    path = "*",
    query = None,
    host = None,
}

test_parse! {
    test_uri_parse_authority_no_port,
    "localhost",
    ["LOCALHOST", "LocaLHOSt"],

    scheme = None,
    authority = part!("localhost"),
    path = "",
    query = None,
    port = None,
    host = Some("localhost"),
}

test_parse! {
    test_uri_authority_only_one_character_issue_197,
    "S",
    [],

    scheme = None,
    authority = part!("S"),
    path = "",
    query = None,
    port = None,
    host = Some("S"),
}

test_parse! {
    test_uri_parse_authority_form,
    "localhost:3000",
    ["localhosT:3000"],

    scheme = None,
    authority = part!("localhost:3000"),
    path = "",
    query = None,
    host = Some("localhost"),
    port = Port::from_str("3000").ok(),
}

test_parse! {
    test_uri_parse_absolute_with_default_port_http,
    "http://127.0.0.1:80",
    ["http://127.0.0.1:80/"],

    scheme = part!("http"),
    authority = part!("127.0.0.1:80"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    port = Port::from_str("80").ok(),
}

test_parse! {
    test_uri_parse_absolute_with_default_port_https,
    "https://127.0.0.1:443",
    ["https://127.0.0.1:443/"],

    scheme = part!("https"),
    authority = part!("127.0.0.1:443"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    port = Port::from_str("443").ok(),
}

test_parse! {
    test_uri_parse_fragment_questionmark,
    "http://127.0.0.1/#?",
    [],

    scheme = part!("http"),
    authority = part!("127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_uri_parse_path_with_terminating_questionmark,
    "http://127.0.0.1/path?",
    [],

    scheme = part!("http"),
    authority = part!("127.0.0.1"),
    path = "/path",
    query = Some(""),
    port = None,
}

test_parse! {
    test_uri_parse_absolute_form_with_empty_path_and_nonempty_query,
    "http://127.0.0.1?foo=bar",
    [],

    scheme = part!("http"),
    authority = part!("127.0.0.1"),
    path = "/",
    query = Some("foo=bar"),
    port = None,
}

test_parse! {
    test_uri_parse_absolute_form_with_empty_path_and_fragment_with_slash,
    "http://127.0.0.1#foo/bar",
    [],

    scheme = part!("http"),
    authority = part!("127.0.0.1"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_uri_parse_absolute_form_with_empty_path_and_fragment_with_questionmark,
    "http://127.0.0.1#foo?bar",
    [],

    scheme = part!("http"),
    authority = part!("127.0.0.1"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_uri_parse_long_host_with_no_scheme,
    "thequickbrownfoxjumpedoverthelazydogtofindthelargedangerousdragon.localhost",
    [],

    scheme = None,
    authority = part!("thequickbrownfoxjumpedoverthelazydogtofindthelargedangerousdragon.localhost"),
    path = "",
    query = None,
    port = None,
}

test_parse! {
    test_uri_parse_long_host_with_port_and_no_scheme,
    "thequickbrownfoxjumpedoverthelazydogtofindthelargedangerousdragon.localhost:1234",
    [],

    scheme = None,
    authority = part!("thequickbrownfoxjumpedoverthelazydogtofindthelargedangerousdragon.localhost:1234"),
    path = "",
    query = None,
    port = Port::from_str("1234").ok(),
}

test_parse! {
    test_userinfo1,
    "http://a:b@127.0.0.1:1234/",
    [],

    scheme = part!("http"),
    authority = part!("a:b@127.0.0.1:1234"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    port = Port::from_str("1234").ok(),
}

test_parse! {
    test_userinfo2,
    "http://a:b@127.0.0.1/",
    [],

    scheme = part!("http"),
    authority = part!("a:b@127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_userinfo3,
    "http://a@127.0.0.1/",
    [],

    scheme = part!("http"),
    authority = part!("a@127.0.0.1"),
    host = Some("127.0.0.1"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_userinfo_with_port,
    "user@localhost:3000",
    [],

    scheme = None,
    authority = part!("user@localhost:3000"),
    path = "",
    query = None,
    host = Some("localhost"),
    port = Port::from_str("3000").ok(),
}

test_parse! {
    test_userinfo_pass_with_port,
    "user:pass@localhost:3000",
    [],

    scheme = None,
    authority = part!("user:pass@localhost:3000"),
    path = "",
    query = None,
    host = Some("localhost"),
    port = Port::from_str("3000").ok(),
}

test_parse! {
    test_ipv6,
    "http://[2001:0db8:85a3:0000:0000:8a2e:0370:7334]/",
    [],

    scheme = part!("http"),
    authority = part!("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]"),
    host = Some("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_ipv6_shorthand,
    "http://[::1]/",
    [],

    scheme = part!("http"),
    authority = part!("[::1]"),
    host = Some("[::1]"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_ipv6_shorthand2,
    "http://[::]/",
    [],

    scheme = part!("http"),
    authority = part!("[::]"),
    host = Some("[::]"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_ipv6_shorthand3,
    "http://[2001:db8::2:1]/",
    [],

    scheme = part!("http"),
    authority = part!("[2001:db8::2:1]"),
    host = Some("[2001:db8::2:1]"),
    path = "/",
    query = None,
    port = None,
}

test_parse! {
    test_ipv6_with_port,
    "http://[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:8008/",
    [],

    scheme = part!("http"),
    authority = part!("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:8008"),
    host = Some("[2001:0db8:85a3:0000:0000:8a2e:0370:7334]"),
    path = "/",
    query = None,
    port = Port::from_str("8008").ok(),
}

test_parse! {
    test_percentage_encoded_path,
    "/echo/abcdefgh_i-j%20/abcdefg_i-j%20478",
    [],

    scheme = None,
    authority = None,
    host = None,
    path = "/echo/abcdefgh_i-j%20/abcdefg_i-j%20478",
    query = None,
    port = None,
}

test_parse! {
    test_path_permissive,
    "/foo=bar|baz\\^~%",
    [],

    path = "/foo=bar|baz\\^~%",
}

test_parse! {
    test_query_permissive,
    "/?foo={bar|baz}\\^`",
    [],

    query = Some("foo={bar|baz}\\^`"),
}

#[test]
fn test_uri_parse_error() {
    fn err(s: &str) {
        Uri::from_str(s).unwrap_err();
    }

    err("http://");
    err("htt:p//host");
    err("hyper.rs/");
    err("hyper.rs?key=val");
    err("?key=val");
    err("localhost/");
    err("localhost?key=val");
    err("\0");
    err("http://[::1");
    err("http://::1]");
    err("localhost:8080:3030");
    err("@");
    err("http://username:password@/wut");

    // illegal queries
    err("/?foo\rbar");
    err("/?foo\nbar");
    err("/?<");
    err("/?>");
}

#[test]
fn test_max_uri_len() {
    let mut uri = vec![];
    uri.extend(b"http://localhost/");
    uri.extend(vec![b'a'; 70 * 1024]);

    let uri = String::from_utf8(uri).unwrap();
    let res: Result<Uri, InvalidUri> = uri.parse();

    assert_eq!(res.unwrap_err().0, ErrorKind::TooLong);
}

#[test]
fn test_overflowing_scheme() {
    let mut uri = vec![];
    uri.extend(vec![b'a'; 256]);
    uri.extend(b"://localhost/");

    let uri = String::from_utf8(uri).unwrap();
    let res: Result<Uri, InvalidUri> = uri.parse();

    assert_eq!(res.unwrap_err().0, ErrorKind::SchemeTooLong);
}

#[test]
fn test_max_length_scheme() {
    let mut uri = vec![];
    uri.extend(vec![b'a'; 64]);
    uri.extend(b"://localhost/");

    let uri = String::from_utf8(uri).unwrap();
    let uri: Uri = uri.parse().unwrap();

    assert_eq!(uri.scheme_str().unwrap().len(), 64);
}

#[test]
fn test_uri_to_path_and_query() {
    let cases = vec![
        ("/", "/"),
        ("/foo?bar", "/foo?bar"),
        ("/foo?bar#nope", "/foo?bar"),
        ("http://hyper.rs", "/"),
        ("http://hyper.rs/", "/"),
        ("http://hyper.rs/path", "/path"),
        ("http://hyper.rs?query", "/?query"),
        ("*", "*"),
    ];

    for case in cases {
        let uri = Uri::from_str(case.0).unwrap();
        let s = uri.path_and_query().unwrap().to_string();

        assert_eq!(s, case.1);
    }
}

#[test]
fn test_authority_uri_parts_round_trip() {
    let s = "hyper.rs";
    let uri = Uri::from_str(s).expect("first parse");
    assert_eq!(uri, s);
    assert_eq!(uri.to_string(), s);

    let parts = uri.into_parts();
    let uri2 = Uri::from_parts(parts).expect("from_parts");
    assert_eq!(uri2, s);
    assert_eq!(uri2.to_string(), s);
}

#[test]
fn test_partial_eq_path_with_terminating_questionmark() {
    let a = "/path";
    let uri = Uri::from_str("/path?").expect("first parse");

    assert_eq!(uri, a);
}
