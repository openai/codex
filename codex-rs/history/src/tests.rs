use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn response_item_envelope_accessors_preserve_item() {
    let expected_item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "hello".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let mut envelope = ResponseItemEnvelope::new(expected_item.clone());

    assert_eq!(&*envelope, &expected_item);
    let borrowed: &ResponseItem = envelope.borrow();
    assert_eq!(borrowed, &expected_item);
    let replacement_item = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "goodbye".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    *envelope = replacement_item.clone();

    assert_eq!(envelope.into_item(), replacement_item);
}

#[test]
/// Keeps legacy response-item rollout lines readable and byte-shape compatible.
fn response_item_rollout_line_preserves_shape() -> Result<()> {
    let legacy_line = json!({
        "timestamp": "2025-01-03T12:00:00.000Z",
        "ordinal": 7,
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "hello",
            }],
        },
    });

    let line = serde_json::from_value::<RolloutLine>(legacy_line.clone())?;
    let RolloutItem::ResponseItem(envelope) = &line.item else {
        panic!("expected response item");
    };
    assert!(matches!(&envelope.item, ResponseItem::Message { .. }));
    assert_eq!(envelope.metadata, None);
    assert_eq!(serde_json::to_value(line)?, legacy_line);
    Ok(())
}

#[test]
/// Keeps harness metadata beside, rather than inside, response-item payloads.
fn response_item_envelope_stores_metadata_beside_rollout_payload() -> Result<()> {
    let response_item = response_message("developer");
    let line = RolloutLine {
        timestamp: "2025-01-03T12:00:00.000Z".to_string(),
        ordinal: Some(7),
        item: RolloutItem::ResponseItem(ResponseItemEnvelope {
            item: response_item.clone(),
            metadata: Some(CodexHarnessMetadata {
                client_authored: true,
            }),
        }),
    };
    let serialized = serde_json::to_value(&line)?;

    assert_eq!(
        serialized,
        json!({
            "timestamp": "2025-01-03T12:00:00.000Z",
            "ordinal": 7,
            "type": "response_item",
            "payload": response_item,
            "metadata": { "client_authored": true },
        })
    );
    assert_eq!(serialized["payload"].get("metadata"), None);

    let restored = serde_json::from_value::<RolloutLine>(serialized)?;
    let RolloutItem::ResponseItem(envelope) = restored.item else {
        panic!("expected response item");
    };
    assert_eq!(
        envelope.metadata,
        Some(CodexHarnessMetadata {
            client_authored: true,
        })
    );
    Ok(())
}

#[test]
/// Keeps future metadata fields from making older binaries reject persisted items.
fn response_item_envelope_ignores_unknown_harness_metadata_fields() -> Result<()> {
    let line = serde_json::from_value::<RolloutLine>(json!({
        "timestamp": "2025-01-03T12:00:00.000Z",
        "ordinal": 7,
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "hello",
            }],
        },
        "metadata": {
            "future_field": "value",
        },
    }))?;

    let RolloutItem::ResponseItem(envelope) = line.item else {
        panic!("expected response item");
    };
    assert_eq!(envelope.metadata, Some(CodexHarnessMetadata::default()));

    let compacted = serde_json::from_value::<CompactedItem>(json!({
        "message": "summary",
        "replacement_history": [response_message("user")],
        "replacement_history_metadata": [{ "future_field": "value" }],
    }))?;
    assert_eq!(
        compacted.replacement_history.expect("replacement history")[0].metadata,
        Some(CodexHarnessMetadata::default())
    );
    Ok(())
}

#[test]
/// Keeps legacy compacted replacement histories readable and shape-compatible.
fn response_item_replacement_history_preserves_shape() -> Result<()> {
    let legacy_item = json!({
        "message": "summary",
        "replacement_history": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "hello",
            }],
        }],
    });

    let item = serde_json::from_value::<CompactedItem>(legacy_item.clone())?;
    let replacement_history = item
        .replacement_history
        .as_ref()
        .expect("replacement history");
    assert!(matches!(
        &replacement_history[0].item,
        ResponseItem::Message { .. }
    ));
    assert_eq!(replacement_history[0].metadata, None);
    assert_eq!(serde_json::to_value(item)?, legacy_item);
    Ok(())
}

#[test]
/// Stores complete aligned checkpoint metadata without modifying response items.
fn compacted_replacement_history_stores_metadata_in_an_aligned_sidecar() -> Result<()> {
    let developer_message = response_message("developer");
    let compaction_item = ResponseItem::Compaction {
        id: None,
        encrypted_content: "opaque".to_string(),
        internal_chat_message_metadata_passthrough: None,
    };
    let item = CompactedItem {
        message: "summary".to_string(),
        replacement_history: Some(vec![
            ResponseItemEnvelope {
                item: developer_message.clone(),
                metadata: Some(CodexHarnessMetadata {
                    client_authored: true,
                }),
            },
            ResponseItemEnvelope::new(compaction_item.clone()),
        ]),
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
    };

    let serialized = serde_json::to_value(item)?;
    assert_eq!(
        serialized,
        json!({
            "message": "summary",
            "replacement_history": [developer_message, compaction_item],
            "replacement_history_metadata": [
                { "client_authored": true },
                { "client_authored": false },
            ],
        })
    );

    let restored = serde_json::from_value::<CompactedItem>(serialized)?;
    assert_eq!(
        restored.replacement_history,
        Some(vec![
            ResponseItemEnvelope {
                item: developer_message,
                metadata: Some(CodexHarnessMetadata {
                    client_authored: true,
                }),
            },
            ResponseItemEnvelope {
                item: compaction_item,
                metadata: Some(CodexHarnessMetadata::default()),
            },
        ])
    );
    Ok(())
}

#[test]
/// Rejects checkpoint sidecars that cannot be paired unambiguously with history.
fn compacted_replacement_history_rejects_misaligned_metadata() {
    let malformed_items = [
        json!({
            "message": "summary",
            "replacement_history": [response_message("user")],
            "replacement_history_metadata": [],
        }),
        json!({
            "message": "summary",
            "replacement_history": [response_message("user")],
            "replacement_history_metadata": [{}, {}],
        }),
        json!({
            "message": "summary",
            "replacement_history_metadata": [{}],
        }),
    ];

    for malformed in malformed_items {
        let error = serde_json::from_value::<CompactedItem>(malformed)
            .expect_err("misaligned checkpoint metadata must be rejected");
        assert!(
            error.to_string().contains("replacement_history_metadata"),
            "error: {error}"
        );
    }
}

#[test]
/// Keeps annotated checkpoints readable by binaries expecting raw response items.
fn compacted_metadata_remains_compatible_with_legacy_response_item_readers() -> Result<()> {
    #[derive(Deserialize)]
    #[serde(tag = "type", content = "payload", rename_all = "snake_case")]
    enum LegacyRolloutItem {
        ResponseItem(Box<ResponseItem>),
        Compacted(LegacyCompactedItem),
    }

    #[derive(Deserialize)]
    struct LegacyCompactedItem {
        replacement_history: Vec<ResponseItem>,
    }

    let response_item = response_message("developer");
    let envelope = ResponseItemEnvelope {
        item: response_item.clone(),
        metadata: Some(CodexHarnessMetadata {
            client_authored: true,
        }),
    };
    let response_line = serde_json::to_value(RolloutItem::ResponseItem(envelope.clone()))?;
    let LegacyRolloutItem::ResponseItem(legacy_response) =
        serde_json::from_value::<LegacyRolloutItem>(response_line)?
    else {
        panic!("expected legacy response item");
    };
    assert_eq!(*legacy_response, response_item);

    let compacted_line = serde_json::to_value(RolloutItem::Compacted(CompactedItem {
        message: "summary".to_string(),
        replacement_history: Some(vec![envelope]),
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
    }))?;

    let LegacyRolloutItem::Compacted(legacy) =
        serde_json::from_value::<LegacyRolloutItem>(compacted_line)?
    else {
        panic!("expected legacy compacted item");
    };
    assert_eq!(legacy.replacement_history, vec![response_item]);
    Ok(())
}

#[test]
/// Preserves the established tagged payload representation for every rollout variant.
fn rollout_item_variants_preserve_existing_payload_shapes() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let fixtures = [
        json!({
            "type": "session_meta",
            "payload": SessionMetaLine {
                meta: SessionMeta::default(),
                git: None,
            },
        }),
        json!({
            "type": "response_item",
            "payload": response_message("user"),
        }),
        json!({
            "type": "inter_agent_communication",
            "payload": {
                "author": "/root",
                "recipient": "/root/child",
                "other_recipients": [],
                "content": "hello",
                "trigger_turn": false,
            },
        }),
        json!({
            "type": "inter_agent_communication_metadata",
            "payload": { "trigger_turn": true },
        }),
        json!({
            "type": "compacted",
            "payload": { "message": "summary" },
        }),
        json!({
            "type": "turn_context",
            "payload": {
                "cwd": cwd,
                "approval_policy": "never",
                "sandbox_policy": { "type": "danger-full-access" },
                "model": "gpt-5",
                "summary": "auto",
            },
        }),
        json!({
            "type": "world_state",
            "payload": { "full": true, "state": { "cwd": "/tmp" } },
        }),
        json!({
            "type": "security_risk_score",
            "payload": {
                "scores": {
                    "action_risk": 0.92,
                    "data_exfiltration": 0.31,
                },
            },
        }),
        json!({
            "type": "event_msg",
            "payload": { "type": "warning", "message": "heads up" },
        }),
    ];

    for expected in fixtures {
        let item = serde_json::from_value::<RolloutItem>(expected.clone())?;
        assert_eq!(serde_json::to_value(item)?, expected);
    }
    Ok(())
}

#[test]
/// Keeps the generated schema aligned with each variant's actual persisted shape.
fn rollout_item_schema_matches_tagged_payload_and_sibling_metadata() -> Result<()> {
    let schema = serde_json::to_value(schemars::schema_for!(RolloutItem))?;
    let variants = schema["oneOf"].as_array().expect("rollout variants");
    assert_eq!(variants.len(), 9);

    for variant in variants {
        let required = variant["required"].as_array().expect("required fields");
        assert!(required.contains(&json!("type")), "schema: {variant}");
        assert!(required.contains(&json!("payload")), "schema: {variant}");
    }

    let response_item = variants
        .iter()
        .find(|variant| variant["properties"]["type"]["enum"] == json!(["response_item"]))
        .expect("response item schema");
    assert!(response_item["properties"].get("metadata").is_some());
    assert_eq!(
        response_item["properties"]["payload"]["$ref"],
        json!("#/definitions/ResponseItem")
    );

    let compacted = &schema["definitions"]["CompactedItem"];
    assert_eq!(
        compacted["properties"]["replacement_history"]["items"]["$ref"],
        json!("#/definitions/ResponseItem")
    );
    assert_eq!(
        compacted["properties"]["replacement_history_metadata"]["items"]["$ref"],
        json!("#/definitions/CodexHarnessMetadata")
    );
    let required = compacted["required"].as_array().expect("required fields");
    assert!(!required.contains(&json!("replacement_history")));
    assert!(!required.contains(&json!("replacement_history_metadata")));
    Ok(())
}

fn response_message(role: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![ContentItem::InputText {
            text: "hello".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[test]
/// Preserves the stored compacted-item window metadata shape.
fn compacted_item_serializes_window_number_and_id() -> Result<()> {
    let item = CompactedItem {
        message: "summary".to_string(),
        replacement_history: None,
        window_number: Some(3),
        first_window_id: Some("019b3f6e-0000-7000-8000-000000000001".to_string()),
        previous_window_id: Some("019b3f6e-0000-7000-8000-000000000002".to_string()),
        window_id: Some("019b3f6e-7a10-7cc3-8b6e-1d09e2f7a001".to_string()),
    };

    assert_eq!(
        serde_json::to_value(item)?,
        json!({
            "message": "summary",
            "window_number": 3,
            "first_window_id": "019b3f6e-0000-7000-8000-000000000001",
            "previous_window_id": "019b3f6e-0000-7000-8000-000000000002",
            "window_id": "019b3f6e-7a10-7cc3-8b6e-1d09e2f7a001",
        })
    );
    Ok(())
}

#[test]
/// Keeps legacy numeric window IDs readable in stored compacted items.
fn compacted_item_migrates_legacy_numeric_window_id() -> Result<()> {
    let item = serde_json::from_value::<CompactedItem>(json!({
        "message": "summary",
        "window_id": 3,
    }))?;

    assert_eq!(
        item,
        CompactedItem {
            message: "summary".to_string(),
            replacement_history: None,
            window_number: Some(3),
            first_window_id: None,
            previous_window_id: None,
            window_id: None,
        }
    );
    Ok(())
}

#[test]
fn copied_history_uses_persisted_history_mode() -> Result<()> {
    let thread_id = ThreadId::from_string("00000000-0000-0000-0000-000000000001")?;
    let session_meta = RolloutItem::SessionMeta(SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            history_mode: ThreadHistoryMode::Legacy,
            ..SessionMeta::default()
        },
        git: None,
    });
    let history = InitialHistory::Resumed(ResumedHistory {
        conversation_id: thread_id,
        history: Arc::new(vec![session_meta.clone()]),
        rollout_path: None,
    });

    assert_eq!(
        history.get_history_mode(ThreadHistoryMode::Paginated),
        ThreadHistoryMode::Legacy
    );
    assert_eq!(
        InitialHistory::Forked(vec![session_meta]).get_history_mode(ThreadHistoryMode::Paginated),
        ThreadHistoryMode::Legacy
    );
    assert_eq!(
        InitialHistory::New.get_history_mode(ThreadHistoryMode::Paginated),
        ThreadHistoryMode::Paginated
    );
    assert_eq!(
        InitialHistory::Resumed(ResumedHistory {
            conversation_id: thread_id,
            history: Arc::new(Vec::new()),
            rollout_path: None,
        })
        .get_history_mode(ThreadHistoryMode::Paginated),
        ThreadHistoryMode::Paginated
    );
    Ok(())
}

#[test]
fn multi_agent_version_uses_newest_present_session_meta_value() -> Result<()> {
    let thread_id = ThreadId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8")?;
    let older_meta = SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            multi_agent_version: Some(MultiAgentVersion::V2),
            ..Default::default()
        },
        git: None,
    };
    let newer_meta_without_version = SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            multi_agent_version: None,
            ..Default::default()
        },
        git: None,
    };

    assert_eq!(
        multi_agent_version_from_items(
            &[
                RolloutItem::SessionMeta(older_meta),
                RolloutItem::SessionMeta(newer_meta_without_version),
            ],
            Some(thread_id),
        ),
        Some(MultiAgentVersion::V2)
    );
    Ok(())
}
