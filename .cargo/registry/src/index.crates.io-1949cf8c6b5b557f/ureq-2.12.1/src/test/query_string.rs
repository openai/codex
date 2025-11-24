use super::Recorder;
use crate::get;

#[test]
fn no_query_string() {
    let recorder = Recorder::register("/no_query_string");
    get("test://host/no_query_string").call().unwrap();
    assert!(recorder.contains("GET /no_query_string HTTP/1.1"))
}

#[test]
fn escaped_query_string() {
    let recorder = Recorder::register("/escaped_query_string");
    get("test://host/escaped_query_string")
        .query("foo", "bar")
        .query("baz", "yo lo")
        .call()
        .unwrap();
    assert!(recorder.contains("GET /escaped_query_string?foo=bar&baz=yo+lo HTTP/1.1"));
}

#[test]
fn query_in_path() {
    let recorder = Recorder::register("/query_in_path");
    get("test://host/query_in_path?foo=bar").call().unwrap();
    assert!(recorder.contains("GET /query_in_path?foo=bar HTTP/1.1"))
}

#[test]
fn query_in_path_and_req() {
    let recorder = Recorder::register("/query_in_path_and_req");
    get("test://host/query_in_path_and_req?foo=bar")
        .query("baz", "1 2 3")
        .call()
        .unwrap();
    assert!(recorder.contains("GET /query_in_path_and_req?foo=bar&baz=1+2+3 HTTP/1.1"));
}
