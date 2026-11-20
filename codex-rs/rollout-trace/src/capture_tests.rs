#![cfg(test)]

use super::*;
use crate::PayloadReader;
use crate::reducer::test_support::ROOT_THREAD_ID;
use crate::replay_bundle;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

pub(crate) fn limited_trace(body: usize, bundle: u64) -> Result<(TempDir, TraceWriter)> {
    let root = TempDir::new()?;
    let writer = TraceWriter::create_with_limits(
        root.path(),
        "t".into(),
        "r".into(),
        ROOT_THREAD_ID.into(),
        TraceCaptureLimits {
            max_payload_bytes: body,
            max_bytes: bundle,
        },
    )?;
    Ok((root, writer))
}

#[test]
fn parts_reuse_the_complete_identity_and_round_trip_in_order() -> Result<()> {
    let (root, writer) = limited_trace(/*body*/ 80, /*bundle*/ 32 << 10)?;
    let prefix = "a".repeat(80);
    let oversized =
        json!({"input": [format!("{prefix}1"), format!("{prefix}2"), format!("{prefix}1")]});
    let first = writer.write_json_payload(RawPayloadKind::PreparedPrompt, &oversized)?;
    let repeated = writer.write_json_payload(RawPayloadKind::PreparedPrompt, &oversized)?;
    let items = &first.parts;
    assert_eq!(first.parts, repeated.parts);
    assert_eq!(items[0], items[2]);
    assert_ne!(items[0].raw_payload_id, items[1].raw_payload_id);
    assert_eq!(
        items[0].capture,
        PayloadCapture::Truncated {
            original_bytes: 83,
            retained_bytes: 80
        }
    );
    let mut reader = PayloadReader::new(root.path());
    assert!(reader.read(&first).is_err());
    let captured = reader.read_prefix(&items[0])?.to_vec();
    assert_eq!(captured, reader.read_prefix(&items[1])?);

    let body = r#"{"z":true,"input":[{"s":"\u0061"},1.0,{"s":"\u0061"}],"a":[]}"#;
    let prompt = writer.write_json_payload(
        RawPayloadKind::PreparedPrompt,
        &RawValue::from_string(body.to_string())?,
    )?;
    assert_eq!(reader.read(&prompt)?, body.as_bytes());
    writer.append(RawTraceEventPayload::ProtocolEventObserved {
        event_type: "test".into(),
        event_payload: first,
    })?;
    drop(writer);
    let state = replay_bundle(root.path())?;
    assert_eq!(
        state.recording_status,
        crate::RecordingStatus::ClosedWithPartialPayloads
    );
    assert_eq!(state.raw_payloads.len(), 3);
    Ok(())
}

#[test]
fn separate_tool_results_reuse_open_content_and_structure_shards() -> Result<()> {
    let (root, writer) = limited_trace(/*body*/ 1 << 20, /*bundle*/ 4 << 20)?;
    let prefix = "x".repeat(96 << 10);
    let mut reader = PayloadReader::new(root.path());
    let mut shard_numbers = None;
    for line in 0..16 {
        let body = json!({"output": format!("{prefix}\n{line}")});
        let parent = writer.write_json_payload(RawPayloadKind::ToolResult, &body)?;
        let [child] = parent.parts.as_slice() else {
            panic!("expected a separate output value");
        };
        let pair = (child.shard.unwrap(), parent.shard.unwrap());
        assert_ne!(pair.0, pair.1);
        assert_eq!(pair, *shard_numbers.get_or_insert(pair));
        writer.append(RawTraceEventPayload::ProtocolEventObserved {
            event_type: "tool_result".into(),
            event_payload: parent.clone(),
        })?;
        assert_eq!(reader.read_json(&parent)?, body);
    }
    assert_eq!(
        replay_bundle(root.path())?.recording_status,
        crate::RecordingStatus::Unclosed
    );
    drop(writer);
    assert_eq!(
        replay_bundle(root.path())?.recording_status,
        crate::RecordingStatus::Complete
    );
    assert_eq!(
        std::fs::read_dir(root.path().join(PAYLOADS_DIR_NAME))?.count(),
        2
    );
    Ok(())
}

#[test]
fn compressed_budget_admits_small_later_events_after_an_omission() -> Result<()> {
    let limit = 4096;
    let (root, writer) = limited_trace(/*body*/ 1 << 20, /*bundle*/ limit)?;
    let compressible =
        writer.write_json_payload(RawPayloadKind::ProtocolEvent, &"x".repeat(8192))?;
    assert!(compressible.is_complete());
    let noisy: String = (0u32..256)
        .map(|i| format!("{:x}", Sha256::digest(i.to_le_bytes())))
        .collect();
    let omitted = writer.write_json_payload(RawPayloadKind::ProtocolEvent, &noisy)?;
    assert_eq!(
        (&omitted.capture, omitted.shard),
        (
            &PayloadCapture::Omitted {
                original_bytes: noisy.len() + 2
            },
            None
        )
    );
    crate::reducer::test_support::start_thread(&writer, ROOT_THREAD_ID, "/root")?;
    writer.append_with_context(
        RawTraceEventContext::turn(ROOT_THREAD_ID, "turn"),
        RawTraceEventPayload::HistoryItemAccepted {
            source_tool_call_id: None,
            item_id: "missing-receipt".into(),
            agent_message_payload: Some(omitted.clone()),
            tool_output_payload: None,
        },
    )?;
    let later = writer.write_json_payload(RawPayloadKind::ToolResult, &json!({"ok": true}))?;
    for event_payload in [compressible, omitted, later.clone()] {
        writer.append(RawTraceEventPayload::ProtocolEventObserved {
            event_type: "test".into(),
            event_payload,
        })?;
    }
    drop(writer);
    let state = replay_bundle(root.path())?;
    assert_eq!(
        state.recording_status,
        crate::RecordingStatus::ClosedWithPartialPayloads
    );
    assert_eq!(
        PayloadReader::new(root.path()).read_json(&later)?,
        json!({"ok": true})
    );
    let mut bytes = std::fs::metadata(root.path().join("manifest.json"))?.len()
        + std::fs::metadata(root.path().join("trace.jsonl"))?.len();
    for entry in std::fs::read_dir(root.path().join(PAYLOADS_DIR_NAME))? {
        bytes += entry?.metadata()?.len();
    }
    assert!(bytes <= limit);
    Ok(())
}

#[test]
fn exhausting_event_space_leaves_a_readable_prefix() -> Result<()> {
    let (root, writer) = limited_trace(/*body*/ 1024, /*bundle*/ 1024)?;
    let event_payload = writer.write_json_payload(RawPayloadKind::ProtocolEvent, &true)?;
    let record = |name: String| RawTraceEventPayload::ProtocolEventObserved {
        event_type: name,
        event_payload: event_payload.clone(),
    };
    writer.append(record("small".into()))?;
    assert!(writer.append(record("x".repeat(1024))).is_err());
    drop(writer);
    let state = replay_bundle(root.path())?;
    assert_eq!(state.recording_status, crate::RecordingStatus::Unclosed);
    assert_eq!(state.raw_payloads.len(), 1);
    Ok(())
}

#[test]
fn repeated_mcp_images_leave_room_for_their_event_and_recording_completion() -> Result<()> {
    let image = json!({"type":"image","mimeType":"image/png","data":"A".repeat(64 << 10)});
    let result = json!({"result":{"Ok":{"content":vec![image; 512]}}});
    let cap = 48 << 10;
    let original_bytes = serde_json::to_vec(&result)?.len();
    for (bundle_bytes, turn, expected_capture) in [
        (
            64 << 10,
            "turn".to_string(),
            PayloadCapture::Truncated {
                original_bytes,
                retained_bytes: cap,
            },
        ),
        (128 << 10, "turn".to_string(), PayloadCapture::Complete),
        // The refs fit by themselves, but the serialized event envelope consumes the difference.
        (
            128 << 10,
            "t".repeat(120 << 10),
            PayloadCapture::Omitted { original_bytes },
        ),
    ] {
        let (root, writer) = limited_trace(cap, bundle_bytes)?;
        writer.event_with_payload(
            RawTraceEventContext::turn(ROOT_THREAD_ID, &turn),
            RawPayloadKind::ToolRuntimeEvent,
            &result,
            |runtime_payload| RawTraceEventPayload::ToolCallRuntimeEnded {
                tool_call_id: "images".into(),
                status: crate::ExecutionStatus::Completed,
                runtime_payload,
            },
        )?;
        let next = RawTraceEventPayload::CodexTurnStarted {
            codex_turn_id: "next".into(),
            thread_id: ROOT_THREAD_ID.into(),
        };
        writer.append(next.clone())?;
        drop(writer);
        let log = std::fs::read_to_string(root.path().join(RAW_EVENT_LOG_FILE_NAME))?;
        let events = log
            .lines()
            .map(serde_json::from_str)
            .collect::<serde_json::Result<Vec<RawTraceEvent>>>()?;
        let [tool, later, closing] = events.as_slice() else {
            panic!("expected tool result, next turn and recording completion");
        };
        let RawTraceEventPayload::ToolCallRuntimeEnded {
            runtime_payload, ..
        } = &tool.payload
        else {
            panic!("missing MCP result");
        };
        assert_eq!(later.payload, next);
        assert_eq!(closing.payload, RawTraceEventPayload::RecordingCompleted);
        assert_eq!(runtime_payload.capture, expected_capture);
        if expected_capture.is_complete() {
            assert_eq!(runtime_payload.parts.len(), 512);
            let first = &runtime_payload.parts[0];
            assert!(runtime_payload.parts.iter().all(|part| part == first));
            assert_eq!(
                PayloadReader::new(root.path()).read_prefix(first)?.len(),
                cap
            );
        } else {
            // No child was written before selecting the compact representation.
            assert!(runtime_payload.parts.is_empty());
            assert_eq!(runtime_payload.raw_payload_id, "raw_payload:1");
        }
        let mut total =
            log.len() as u64 + std::fs::metadata(root.path().join(MANIFEST_FILE_NAME))?.len();
        for shard in std::fs::read_dir(root.path().join(PAYLOADS_DIR_NAME))? {
            total += shard?.metadata()?.len();
        }
        assert!(total <= bundle_bytes);
    }
    Ok(())
}

#[test]
fn tool_and_cell_results_keep_distinct_parents_and_share_the_same_image() -> Result<()> {
    let image = format!("data:image/png;base64,{}", "A".repeat(96 << 10));
    let block = json!({"type":"input_image", "image_url":image});
    let cell = json!({"response":{"Result":{"content_items":[block],"error_text":null}}});
    let inner =
        json!({"type":"code_mode_response","value":{"image_url":image,"detail":"original"}});
    let outer = json!({"type":"direct_response","response_item":{"type":"custom_tool_call_output","output":[block]}});
    for cap in [4096, 128 << 10] {
        let (bundle, writer) = limited_trace(cap, /*bundle*/ 1 << 20)?;
        let mut reader = PayloadReader::new(bundle.path());
        let mut parents = std::collections::HashSet::new();
        let mut children = std::collections::HashSet::new();
        for body in [&inner, &cell, &cell, &outer] {
            let reference = writer.write_json_payload(RawPayloadKind::ToolResult, body)?;
            let [child] = reference.parts.as_slice() else {
                panic!("expected one image reference")
            };
            parents.insert(reference.raw_payload_id.clone());
            children.insert(child.raw_payload_id.clone());
            if image.len() < cap {
                assert_eq!(reader.read(&reference)?, serde_json::to_vec(body)?);
                assert_eq!(reader.read(child)?, serde_json::to_vec(&image)?);
            } else {
                assert!(reference.capture.is_complete());
                assert_eq!(
                    reader.read_prefix(child)?,
                    &serde_json::to_vec(&image)?[..cap]
                );
                assert!(reader.read(&reference).is_err());
            }
        }
        assert_eq!((parents.len(), children.len()), (4, 1));
    }
    Ok(())
}

#[test]
fn arbitrary_tool_strings_share_bytes_and_gaps_have_exact_positions() -> Result<()> {
    let (root, writer) = limited_trace(/*body*/ 4096, /*bundle*/ 1 << 20)?;
    let data = "x".repeat(96 << 10);
    let literal = json!({"$trace_unavailable":"raw_payload:1"});
    let body = json!({
        "structuredContent": {"unknown/field": [literal, null, data]},
        "another": data,
        "path": "payloads/000001.zst",
        "id": "raw_payload:1"
    });
    let reference = writer.write_json_payload(RawPayloadKind::ToolRuntimeEvent, &body)?;
    let [first, second] = reference.parts.as_slice() else {
        panic!("expected two slots")
    };
    assert_eq!(first, second);
    let mut reader = PayloadReader::new(root.path());
    assert!(reader.read(&reference).is_err());
    let observed = reader.observed_json(&reference)?;
    assert_eq!(
        observed.value,
        json!({
            "structuredContent": {"unknown/field": [literal, null, null]},
            "another": null, "path": "payloads/000001.zst", "id": "raw_payload:1"
        })
    );
    assert_eq!(
        observed.missing,
        json!({
            "structuredContent": {"unknown/field": [null, null, first.raw_payload_id]},
            "another": first.raw_payload_id
        })
    );
    Ok(())
}

#[test]
fn tool_string_selection_preserves_json_tokens_and_respects_the_body_limit() -> Result<()> {
    let (root, writer) = limited_trace(/*body*/ 256 << 10, /*bundle*/ 1 << 20)?;
    let text = "x".repeat((64 << 10) - 2);
    let short = &text[1..];
    // Duplicate and escaped keys are legal in raw JSON. Only value tokens are shared.
    let bytes = format!(
        r#"{{"{text}" : "{text}", "a\u002fb":[ "{short}", "{text}"], "a\u002fb": "before\"after"}}"#
    );
    let raw = RawValue::from_string(bytes.clone())?;
    let reference = writer.write_json_payload(RawPayloadKind::ToolResult, &raw)?;
    assert_eq!(reference.parts.len(), 2);
    assert_eq!(reference.parts[0], reference.parts[1]);
    assert_eq!(
        PayloadReader::new(root.path()).read(&reference)?,
        bytes.as_bytes()
    );

    let (root, writer) = limited_trace(/*body*/ 4096, /*bundle*/ 1 << 20)?;
    let fallback = writer.write_json_payload(RawPayloadKind::ToolResult, &raw)?;
    assert!(fallback.parts.is_empty()); // The key alone exceeds the cap.
    assert_eq!(fallback.raw_payload_id, "raw_payload:1");
    assert_eq!(
        PayloadReader::new(root.path()).read_prefix(&fallback)?,
        &bytes.as_bytes()[..4096]
    );
    Ok(())
}
