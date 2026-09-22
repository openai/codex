use anyhow::Result;
use codex_protocol::models::ConfigurationReasoning;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::ThreadSettingsSnapshot;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn thread_settings_snapshot(disabled_plugin_ids: Vec<String>) -> Result<ThreadSettingsSnapshot> {
    Ok(serde_json::from_value(json!({
        "model": "gpt-5",
        "model_provider_id": "openai",
        "approval_policy": "never",
        "approvals_reviewer": "user",
        "permission_profile": codex_protocol::models::PermissionProfile::read_only(),
        "cwd": std::env::current_dir()?,
        "collaboration_mode": {
            "mode": "default",
            "settings": {
                "model": "gpt-5",
                "reasoning_effort": null,
                "developer_instructions": null
            }
        },
        "disabled_plugin_ids": disabled_plugin_ids
    }))?)
}

#[test]
fn latest_disabled_plugin_ids_preserves_owned_updates_and_clears() -> Result<()> {
    let thread_id = ThreadId::new();
    let ancestor_id = ThreadId::new();
    let selected = thread_settings_snapshot(vec!["example@marketplace".to_string()])?;
    let cleared = thread_settings_snapshot(Vec::new())?;
    let item = |thread_id, thread_settings| {
        RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(
            codex_protocol::protocol::ThreadSettingsAppliedEvent {
                thread_id,
                thread_settings,
            },
        ))
    };
    let foreign_items = vec![
        item(Some(ancestor_id), selected.clone()),
        item(/*thread_id*/ None, selected.clone()),
    ];
    assert_eq!(latest_disabled_plugin_ids(&foreign_items, thread_id), None);

    let mut history = vec![item(Some(thread_id), selected.clone())];
    history.extend(foreign_items.clone());
    assert_eq!(
        latest_disabled_plugin_ids(&history, thread_id),
        Some(selected.disabled_plugin_ids.as_slice())
    );

    history.push(item(Some(thread_id), cleared.clone()));
    history.extend(foreign_items);
    history.push(turn_context_with_disabled_plugins(Some(
        selected.disabled_plugin_ids,
    ))?);
    assert_eq!(
        latest_disabled_plugin_ids(&history, thread_id),
        Some(cleared.disabled_plugin_ids.as_slice())
    );
    Ok(())
}

fn turn_context_with_disabled_plugins(
    disabled_plugin_ids: Option<Vec<String>>,
) -> Result<RolloutItem> {
    let mut context: TurnContextItem = serde_json::from_value(json!({
        "cwd": std::env::current_dir()?,
        "approval_policy": "never",
        "sandbox_policy": { "type": "danger-full-access" },
        "model": "gpt-5",
        "summary": "auto"
    }))?;
    context.disabled_plugin_ids = disabled_plugin_ids;
    Ok(RolloutItem::TurnContext(context))
}

#[test]
fn latest_disabled_plugin_ids_falls_back_only_to_latest_turn_context() -> Result<()> {
    let thread_id = ThreadId::new();
    let selected = vec!["example@marketplace".to_string()];
    let mut history = vec![turn_context_with_disabled_plugins(Some(selected.clone()))?];
    assert_eq!(
        latest_disabled_plugin_ids(&history, thread_id),
        Some(selected.as_slice())
    );

    history.push(RolloutItem::EventMsg(EventMsg::ThreadSettingsApplied(
        codex_protocol::protocol::ThreadSettingsAppliedEvent {
            thread_id: Some(ThreadId::new()),
            thread_settings: thread_settings_snapshot(Vec::new())?,
        },
    )));
    assert_eq!(
        latest_disabled_plugin_ids(&history, thread_id),
        Some(selected.as_slice())
    );

    let cleared = Vec::new();
    history.push(turn_context_with_disabled_plugins(Some(cleared.clone()))?);
    assert_eq!(
        latest_disabled_plugin_ids(&history, thread_id),
        Some(cleared.as_slice())
    );

    history.push(turn_context_with_disabled_plugins(
        /*disabled_plugin_ids*/ None,
    )?);
    assert_eq!(latest_disabled_plugin_ids(&history, thread_id), None);
    Ok(())
}

#[test]
fn older_thread_settings_snapshot_defaults_disabled_plugins_to_empty() -> Result<()> {
    let expected = thread_settings_snapshot(Vec::new())?;
    let mut legacy = serde_json::to_value(&expected)?;
    legacy
        .as_object_mut()
        .unwrap()
        .remove("disabled_plugin_ids");

    assert_eq!(
        serde_json::from_value::<ThreadSettingsSnapshot>(legacy)?,
        expected
    );
    Ok(())
}

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
fn mcp_checkpoint_handles_missing_or_unknown_identity() -> Result<()> {
    let metadata: CodexHarnessMetadata = serde_json::from_value(json!({}))?;
    assert_eq!(metadata.mcp_attribution, None);

    let restored: CodexHarnessMetadata = serde_json::from_value(json!({
        "mcp_attribution": {
            "status": "complete",
            "sources": [{
                "server_name": "example",
                "tool_name": "search",
                "first_turn_id": "turn_1",
                "future_identity": "unknown to this reader"
            }]
        }
    }))?;
    assert_eq!(
        restored.mcp_attribution,
        Some(McpAttribution {
            status: McpAttributionStatus::AttributionError,
            sources: Vec::new(),
        })
    );
    Ok(())
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

    let line = RolloutLine {
        timestamp: "2025-01-03T12:00:00.000Z".to_string(),
        ordinal: Some(7),
        item: serde_json::from_value(legacy_line.clone())?,
    };
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
                history_truncation_token_limit: Some(20_000),
                inherited_user_message: true,
                ..Default::default()
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
            "metadata": {
                "client_authored": true,
                "fallback_token_limit_override": 20_000,
                "inherited_user_message": true,
            },
        })
    );
    assert_eq!(serialized["payload"].get("metadata"), None);

    let restored = serde_json::from_value(serialized)?;
    let RolloutItem::ResponseItem(envelope) = restored else {
        panic!("expected response item");
    };
    assert_eq!(
        envelope.metadata,
        Some(CodexHarnessMetadata {
            client_authored: true,
            history_truncation_token_limit: Some(20_000),
            inherited_user_message: true,
            ..Default::default()
        })
    );
    Ok(())
}

#[test]
fn response_item_envelope_preserves_harness_authored_configuration_provenance() -> Result<()> {
    let response_item = ResponseItem::ConfigurationUpdate {
        reasoning: ConfigurationReasoning {
            effort: ReasoningEffort::High,
        },
    };
    let metadata = CodexHarnessMetadata {
        harness_authored_configuration: true,
        ..Default::default()
    };
    let rollout_item = RolloutItem::ResponseItem(ResponseItemEnvelope {
        item: response_item.clone(),
        metadata: Some(metadata.clone()),
    });

    let serialized = serde_json::to_value(&rollout_item)?;
    assert_eq!(
        serialized,
        json!({
            "type": "response_item",
            "payload": {
                "type": "configuration_update",
                "reasoning": { "effort": "high" },
            },
            "metadata": {
                "client_authored": false,
                "harness_authored_configuration": true,
            },
        })
    );

    let restored = serde_json::from_value::<RolloutItem>(serialized)?;
    let RolloutItem::ResponseItem(envelope) = restored else {
        panic!("expected response item");
    };
    assert_eq!(
        envelope,
        ResponseItemEnvelope {
            item: response_item,
            metadata: Some(metadata),
        }
    );
    Ok(())
}

#[test]
/// Keeps future metadata fields from making older binaries reject persisted items.
fn response_item_envelope_ignores_unknown_harness_metadata_fields() -> Result<()> {
    let line = serde_json::from_value(json!({
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

    let RolloutItem::ResponseItem(envelope) = line else {
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
/// Keeps legacy compacted replacement histories readable while adding nullable checkpoints.
fn response_item_replacement_history_accepts_legacy_shape() -> Result<()> {
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
    let mut expected_item = legacy_item;
    expected_item["compaction_response_id"] = json!(null);
    expected_item["latest_token_usage_record"] = json!(null);
    assert_eq!(serde_json::to_value(item)?, expected_item);
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
                    ..Default::default()
                }),
            },
            ResponseItemEnvelope::new(compaction_item.clone()),
        ]),
        retained_context: None,
        guardian_history: None,
        mcp_resource_origins: None,
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
        compaction_response_id: None,
        latest_token_usage_record: None,
        resume_metadata: None,
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
            "compaction_response_id": null,
            "latest_token_usage_record": null,
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
                    ..Default::default()
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
fn compacted_resume_metadata_presence_round_trips_empty_values() -> Result<()> {
    let resume_metadata = CompactionResumeMetadata {
        multi_agent_version: None,
        last_started_turn_id: None,
        previous_turn_settings: None,
    };
    let item = CompactedItem {
        message: "summary".to_string(),
        replacement_history: None,
        retained_context: None,
        guardian_history: None,
        mcp_resource_origins: None,
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
        compaction_response_id: None,
        latest_token_usage_record: None,
        resume_metadata: Some(resume_metadata.clone()),
    };

    let serialized = serde_json::to_value(&item)?;
    assert_eq!(serialized["resume_metadata"], json!(resume_metadata));
    assert_eq!(serde_json::from_value::<CompactedItem>(serialized)?, item);
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
            ..Default::default()
        }),
    };
    let response_line = serde_json::to_value(RolloutItem::ResponseItem(envelope.clone()))?;
    let LegacyRolloutItem::ResponseItem(legacy_response) =
        serde_json::from_value::<LegacyRolloutItem>(response_line)?
    else {
        panic!("expected legacy response item");
    };
    assert_eq!(*legacy_response, response_item);

    let checkpoint = crate::GuardianHistoryCheckpoint(vec![response_item.clone()]);
    let compacted_line = serde_json::to_value(RolloutItem::Compacted(CompactedItem {
        message: "summary".to_string(),
        replacement_history: Some(vec![envelope]),
        retained_context: None,
        guardian_history: Some(checkpoint.clone()),
        mcp_resource_origins: Some(McpResourceOriginCheckpoint::default()),
        window_number: None,
        first_window_id: None,
        previous_window_id: None,
        window_id: None,
        compaction_response_id: None,
        latest_token_usage_record: None,
        resume_metadata: Some(CompactionResumeMetadata {
            multi_agent_version: Some(MultiAgentVersion::V2),
            last_started_turn_id: Some("turn-1".to_string()),
            previous_turn_settings: None,
        }),
    }))?;

    let restored: RolloutItem = serde_json::from_value(compacted_line.clone())?;
    let RolloutItem::Compacted(restored) = restored else {
        panic!("expected compacted item");
    };
    assert_eq!(restored.guardian_history, Some(checkpoint));
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
            "payload": {
                "message": "summary",
                "compaction_response_id": null,
                "latest_token_usage_record": null,
            },
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
            "type": "token_usage_record",
            "payload": {
                "thread_id": "0195cda5-433d-7f9a-9d7b-a9f15b60c2e2",
                "turn_id": "turn-1",
                "session_id": "0195cda5-433d-7f9a-9d7b-a9f15b60c2e2",
                "root_turn_id": "turn-1",
                "response_id": "response-1",
                "usage": {
                    "input_tokens": 10,
                    "cached_input_tokens": 2,
                    "cache_write_input_tokens": 0,
                    "output_tokens": 3,
                    "reasoning_output_tokens": 1,
                    "total_tokens": 13,
                },
                "turn_token_usage": {
                    "input_tokens": 10,
                    "cached_input_tokens": 2,
                    "cache_write_input_tokens": 0,
                    "output_tokens": 3,
                    "reasoning_output_tokens": 1,
                    "total_tokens": 13,
                },
                "thread_token_usage": {
                    "input_tokens": 10,
                    "cached_input_tokens": 2,
                    "cache_write_input_tokens": 0,
                    "output_tokens": 3,
                    "reasoning_output_tokens": 1,
                    "total_tokens": 13,
                },
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
        json!({
            "type": "retained_context",
            "payload": {
                "type": "verified_answer",
                "turn_id": "turn-1",
                "call_id": "ask-1",
                "questions": [{"question": "Publish?", "answer": "Only privately."}],
            },
        }),
        json!({
            "type": "realtime_item",
            "payload": {
                "id": "segment-1",
                "realtime_session_id": "session-1",
                "type": "transcript_segment",
                "role": "assistant",
                "text": "hello",
            },
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
    assert_eq!(variants.len(), 12);

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
        retained_context: None,
        guardian_history: None,
        mcp_resource_origins: None,
        window_number: Some(3),
        first_window_id: Some("019b3f6e-0000-7000-8000-000000000001".to_string()),
        previous_window_id: Some("019b3f6e-0000-7000-8000-000000000002".to_string()),
        window_id: Some("019b3f6e-7a10-7cc3-8b6e-1d09e2f7a001".to_string()),
        compaction_response_id: None,
        latest_token_usage_record: None,
        resume_metadata: None,
    };

    assert_eq!(
        serde_json::to_value(item)?,
        json!({
            "message": "summary",
            "window_number": 3,
            "first_window_id": "019b3f6e-0000-7000-8000-000000000001",
            "previous_window_id": "019b3f6e-0000-7000-8000-000000000002",
            "window_id": "019b3f6e-7a10-7cc3-8b6e-1d09e2f7a001",
            "compaction_response_id": null,
            "latest_token_usage_record": null,
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
            retained_context: None,
            guardian_history: None,
            mcp_resource_origins: None,
            window_number: Some(3),
            first_window_id: None,
            previous_window_id: None,
            window_id: None,
            compaction_response_id: None,
            latest_token_usage_record: None,
            resume_metadata: None,
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
