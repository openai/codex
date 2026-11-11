use pretty_assertions::assert_eq;
use serde_json::json;
use uuid::Uuid;

use super::ResponseItemId;
use crate::models::ResponseItem;

#[test]
fn creates_prefixed_uuid_v7_ids() {
    let id = crate::uuid_v7_item_id_generator_for_test()("msg");
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
fn structured_ids_round_trip_but_raw_items_only_accept_strings() {
    let model_id = serde_json::from_value(json!({"salt":"item","ordinal":7,"value":42})).unwrap();
    let id = ResponseItemId::generated("msg", model_id);
    let mut stored = serde_json::to_value(id).unwrap();
    assert_eq!(
        stored,
        json!({"id":"msg_item042","id_allocation":{
            "prefix":"msg","salt":"item","ordinal":7,"value":42
        }})
    );
    stored["id"] = json!("msg_item042_2");
    stored["id_allocation"]["suffix"] = json!("_2");
    let id: ResponseItemId = serde_json::from_value(stored.clone()).unwrap();
    assert_eq!(id.as_str(), "msg_item042_2");
    assert_eq!(serde_json::to_value(&id).unwrap(), stored);

    let mut raw = json!({"type": "message", "role": "assistant", "content": [], "id": stored});
    assert!(serde_json::from_value::<ResponseItem>(raw.clone()).is_err());
    raw["id"] = json!(id.as_str());
    let item: ResponseItem = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(item.id().unwrap().allocation(), None);
    assert_eq!(serde_json::to_value(item).unwrap(), raw);
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
