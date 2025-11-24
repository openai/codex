use crate::test::Recorder;

use super::super::*;

#[test]
fn content_length_on_str() {
    let recorder = Recorder::register("/content_length_on_str");
    post("test://host/content_length_on_str")
        .send_string("Hello World!!!")
        .unwrap();
    assert!(recorder.contains("\r\nContent-Length: 14\r\n"));
}

#[test]
fn user_set_content_length_on_str() {
    let recorder = Recorder::register("/user_set_content_length_on_str");
    post("test://host/user_set_content_length_on_str")
        .set("Content-Length", "12345")
        .send_string("Hello World!!!")
        .unwrap();
    assert!(recorder.contains("\r\nContent-Length: 12345\r\n"));
}

#[test]
#[cfg(feature = "json")]
fn content_length_on_json() {
    let recorder = Recorder::register("/content_length_on_json");
    let mut json = serde_json::Map::new();
    json.insert(
        "Hello".to_string(),
        serde_json::Value::String("World!!!".to_string()),
    );
    post("test://host/content_length_on_json")
        .send_json(serde_json::Value::Object(json))
        .unwrap();
    assert!(recorder.contains("\r\nContent-Length: 20\r\n"));
}

#[test]
fn content_length_and_chunked() {
    let recorder = Recorder::register("/content_length_and_chunked");
    post("test://host/content_length_and_chunked")
        .set("Transfer-Encoding", "chunked")
        .send_string("Hello World!!!")
        .unwrap();
    assert!(recorder.contains("Transfer-Encoding: chunked\r\n"));
    assert!(!recorder.contains("\r\nContent-Length:\r\n"));
}

#[test]
#[cfg(feature = "charset")]
fn str_with_encoding() {
    let recorder = Recorder::register("/str_with_encoding");
    post("test://host/str_with_encoding")
        .set("Content-Type", "text/plain; charset=iso-8859-1")
        .send_string("Hällo Wörld!!!")
        .unwrap();
    let vec = recorder.to_vec();
    assert_eq!(
        &vec[vec.len() - 14..],
        //H  ä    l    l    o    _   W   ö    r    l    d    !   !   !
        [72, 228, 108, 108, 111, 32, 87, 246, 114, 108, 100, 33, 33, 33]
    );
}

#[test]
#[cfg(feature = "json")]
fn content_type_on_json() {
    let recorder = Recorder::register("/content_type_on_json");
    let mut json = serde_json::Map::new();
    json.insert(
        "Hello".to_string(),
        serde_json::Value::String("World!!!".to_string()),
    );
    post("test://host/content_type_on_json")
        .send_json(serde_json::Value::Object(json))
        .unwrap();
    assert!(recorder.contains("\r\nContent-Type: application/json\r\n"));
}

#[test]
#[cfg(feature = "json")]
fn content_type_not_overriden_on_json() {
    let recorder = Recorder::register("/content_type_not_overriden_on_json");
    let mut json = serde_json::Map::new();
    json.insert(
        "Hello".to_string(),
        serde_json::Value::String("World!!!".to_string()),
    );
    post("test://host/content_type_not_overriden_on_json")
        .set("content-type", "text/plain")
        .send_json(serde_json::Value::Object(json))
        .unwrap();
    assert!(recorder.contains("\r\ncontent-type: text/plain\r\n"));
}
