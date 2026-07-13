use pretty_assertions::assert_eq;
use uuid::Uuid;

use super::ResponseItemId;

#[test]
fn creates_prefixed_uuid_v7_ids() {
    let id = ResponseItemId::new("msg");
    let uuid = id.strip_prefix("msg_").expect("message prefix");
    assert_eq!(
        Uuid::parse_str(uuid).expect("UUID suffix").get_version(),
        Some(uuid::Version::SortRand)
    );
}

#[test]
fn creates_prefixed_ids_with_explicit_suffix() {
    let id = ResponseItemId::with_suffix("msg", "test");
    assert_eq!(id.as_str(), "msg_test");
    assert_eq!(id.to_string(), "msg_test");
    assert_eq!(String::from(id), "msg_test");
}

#[test]
fn accepts_server_ids_verbatim() {
    let id = ResponseItemId::from_server("legacy-id".to_string());
    assert_eq!(id.as_str(), "legacy-id");
}

#[test]
fn deserializes_arbitrary_ids_as_strings() {
    let id: ResponseItemId = serde_json::from_str("\"legacy-id\"").expect("id");
    assert_eq!(id.as_str(), "legacy-id");
    assert_eq!(
        serde_json::to_string(&id).expect("serialized id"),
        "\"legacy-id\""
    );
}

#[test]
fn recognizes_non_empty_prefix_and_suffix() {
    for (value, expected) in [
        ("msg_test", true),
        ("legacy-id", false),
        ("", false),
        ("_test", false),
        ("msg_", false),
    ] {
        let id: ResponseItemId =
            serde_json::from_value(value.into()).expect("ID should deserialize");
        assert_eq!(id.is_prefixed(), expected, "{value}");
    }
}
