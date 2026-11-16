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
            item_id: "missing-receipt".into(),
            agent_message_payload: Some(omitted.clone()),
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
