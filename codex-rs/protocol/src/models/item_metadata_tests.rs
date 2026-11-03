use super::ContentItemKind;
use crate::models::ContentItem;
use crate::models::InternalChatMessageMetadataPassthrough;
use crate::models::ResponseItem;
use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[test]
fn content_kinds_round_trip_without_restricting_future_values() -> Result<()> {
    let item = response_item(InternalChatMessageMetadataPassthrough {
        turn_id: Some("turn-1".to_string()),
        content_item_kinds: Some(vec![ContentItemKind("future_content_kind".to_string())]),
        ..Default::default()
    });

    let serialized = serde_json::to_value(&item)?;
    assert_eq!(
        serialized,
        json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "hello"}],
            "internal_chat_message_metadata_passthrough": {
                "turn_id": "turn-1",
                "content_item_kinds": ["future_content_kind"],
            },
        })
    );
    assert_eq!(serde_json::from_value::<ResponseItem>(serialized)?, item);

    Ok(())
}

#[test]
fn malformed_content_kinds_do_not_prevent_loading_response_item() -> Result<()> {
    for malformed_kinds in [
        json!("user_text"),
        json!({"kind": "user_text"}),
        json!(["user_text", 123]),
    ] {
        let item = serde_json::from_value::<ResponseItem>(response_item_json(json!({
            "turn_id": "turn-1",
            "content_item_kinds": malformed_kinds,
        })))?;

        assert_eq!(
            item,
            response_item(InternalChatMessageMetadataPassthrough {
                turn_id: Some("turn-1".to_string()),
                ..Default::default()
            })
        );
    }

    Ok(())
}

#[test]
fn malformed_content_token_counts_do_not_prevent_loading_response_item() -> Result<()> {
    for value in [json!(-1), json!(true), json!(1.5), json!("4"), json!(null)] {
        let item: ResponseItem = serde_json::from_value(response_item_json(json!({
            "content_token_count": value,
        })))?;
        assert_eq!(
            item,
            response_item(InternalChatMessageMetadataPassthrough::default())
        );
    }
    Ok(())
}

#[test]
fn content_token_counts_are_not_replayed_into_requests() -> Result<()> {
    let item: ResponseItem = serde_json::from_value(response_item_json(json!({
        "turn_id": "turn-1", "content_token_count": 23,
    })))?;
    let ResponseItem::Message {
        internal_chat_message_metadata_passthrough: Some(ref metadata),
        ..
    } = item
    else {
        panic!("expected message metadata");
    };
    assert_eq!(metadata.content_token_count, Some(23));
    let encoded = serde_json::to_value(item)?;
    assert_eq!(
        encoded["internal_chat_message_metadata_passthrough"],
        json!({"turn_id": "turn-1"})
    );
    Ok(())
}

fn response_item(metadata: InternalChatMessageMetadataPassthrough) -> ResponseItem {
    ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "hello".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: Some(metadata),
    }
}

fn response_item_json(metadata: Value) -> Value {
    json!({
        "type": "message",
        "role": "user",
        "content": [{"type": "input_text", "text": "hello"}],
        "internal_chat_message_metadata_passthrough": metadata,
    })
}
