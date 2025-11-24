use crate::test;
use std::io::Read;

use super::{super::*, Recorder};

#[test]
fn header_passing() {
    test::set_handler("/header_passing", |unit| {
        assert!(unit.has("X-Foo"));
        assert_eq!(unit.header("X-Foo").unwrap(), "bar");
        test::make_response(200, "OK", vec!["X-Bar: foo"], vec![])
    });
    let resp = get("test://host/header_passing")
        .set("X-Foo", "bar")
        .call()
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.has("X-Bar"));
    assert_eq!(resp.header("X-Bar").unwrap(), "foo");
}

#[test]
fn repeat_non_x_header() {
    test::set_handler("/repeat_non_x_header", |unit| {
        assert!(unit.has("Accept"));
        assert_eq!(unit.header("Accept").unwrap(), "baz");
        test::make_response(200, "OK", vec![], vec![])
    });
    let resp = get("test://host/repeat_non_x_header")
        .set("Accept", "bar")
        .set("Accept", "baz")
        .call()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
fn repeat_x_header() {
    test::set_handler("/repeat_x_header", |unit| {
        assert!(unit.has("X-Forwarded-For"));
        assert_eq!(unit.header("X-Forwarded-For").unwrap(), "130.240.19.2");
        assert_eq!(
            unit.all("X-Forwarded-For"),
            vec!["130.240.19.2", "130.240.19.3"]
        );
        test::make_response(200, "OK", vec![], vec![])
    });
    let resp = get("test://host/repeat_x_header")
        .set("X-Forwarded-For", "130.240.19.2")
        .set("X-Forwarded-For", "130.240.19.3")
        .call()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
fn body_as_text() {
    test::set_handler("/body_as_text", |_unit| {
        test::make_response(200, "OK", vec![], "Hello World!".to_string().into_bytes())
    });
    let resp = get("test://host/body_as_text").call().unwrap();
    let text = resp.into_string().unwrap();
    assert_eq!(text, "Hello World!");
}

#[test]
#[cfg(feature = "json")]
fn body_as_json() {
    test::set_handler("/body_as_json", |_unit| {
        test::make_response(
            200,
            "OK",
            vec![],
            "{\"hello\":\"world\"}".to_string().into_bytes(),
        )
    });
    let resp = get("test://host/body_as_json").call().unwrap();
    let json: serde_json::Value = resp.into_json().unwrap();
    assert_eq!(json["hello"], "world");
}

#[test]
#[cfg(feature = "json")]
fn body_as_json_deserialize() {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Hello {
        hello: String,
    }

    test::set_handler("/body_as_json_deserialize", |_unit| {
        test::make_response(
            200,
            "OK",
            vec![],
            "{\"hello\":\"world\"}".to_string().into_bytes(),
        )
    });
    let resp = get("test://host/body_as_json_deserialize").call().unwrap();
    let json: Hello = resp.into_json().unwrap();
    assert_eq!(json.hello, "world");
}

#[test]
fn body_as_reader() {
    test::set_handler("/body_as_reader", |_unit| {
        test::make_response(200, "OK", vec![], "abcdefgh".to_string().into_bytes())
    });
    let resp = get("test://host/body_as_reader").call().unwrap();
    let mut reader = resp.into_reader();
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "abcdefgh");
}

#[test]
fn escape_path() {
    let recorder = Recorder::register("/escape_path%20here");
    get("test://host/escape_path here").call().unwrap();
    assert!(recorder.contains("GET /escape_path%20here HTTP/1.1"))
}

#[test]
fn request_debug() {
    let req = get("http://localhost/my/page")
        .set("Authorization", "abcdef")
        .set("Content-Length", "1234")
        .set("Content-Type", "application/json");

    let s = format!("{:?}", req);

    assert_eq!(
        s,
        "Request(GET http://localhost/my/page, [Authorization: abcdef, \
         Content-Length: 1234, Content-Type: application/json])"
    );

    let req = get("http://localhost/my/page?q=z")
        .query("foo", "bar baz")
        .set("Authorization", "abcdef");

    let s = format!("{:?}", req);

    assert_eq!(
        s,
        "Request(GET http://localhost/my/page?q=z&foo=bar+baz, [Authorization: abcdef])"
    );
}

#[test]
fn non_ascii_header() {
    test::set_handler("/non_ascii_header", |_unit| {
        test::make_response(200, "OK", vec!["Wörse: Hädör"], vec![])
    });
    let result = get("test://host/non_ascii_header")
        .set("Bäd", "Headör")
        .call();
    assert!(
        matches!(result, Err(ref e) if e.kind() == ErrorKind::BadHeader),
        "expected Err(BadHeader), got {:?}",
        result
    );
}

#[test]
pub fn no_status_text() {
    // this one doesn't return the status text
    // let resp = get("https://www.okex.com/api/spot/v3/products")
    test::set_handler("/no_status_text", |_unit| {
        test::make_response(200, "", vec![], vec![])
    });
    let resp = get("test://host/no_status_text").call().unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
pub fn header_with_spaces_before_value() {
    test::set_handler("/space_before_value", |unit| {
        assert!(unit.has("X-Test"));
        assert_eq!(unit.header("X-Test").unwrap(), "value");
        test::make_response(200, "OK", vec![], vec![])
    });
    let resp = get("test://host/space_before_value")
        .set("X-Test", "     value")
        .call()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
pub fn host_no_port() {
    let recorder = Recorder::register("/host_no_port");
    get("test://myhost/host_no_port").call().unwrap();
    assert!(recorder.contains("\r\nHost: myhost\r\n"));
}

#[test]
pub fn host_with_port() {
    let recorder = Recorder::register("/host_with_port");
    get("test://myhost:234/host_with_port").call().unwrap();
    assert!(recorder.contains("\r\nHost: myhost:234\r\n"));
}
