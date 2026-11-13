#![cfg(test)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;

use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::Serializer;
use tempfile::TempDir;

use crate::InferenceTraceAttempt;
use crate::InferenceTraceContext;
use crate::PayloadReader;
use crate::RawPayloadKind;
use crate::RawTraceEvent;
use crate::RawTraceEventContext;
use crate::RawTraceEventPayload;
use crate::TraceWriter;

fn recording(dir: &TempDir) -> anyhow::Result<(Arc<TraceWriter>, InferenceTraceAttempt)> {
    let writer = Arc::new(TraceWriter::create(
        dir.path(),
        "trace".into(),
        "rollout".into(),
        "root".into(),
    )?);
    let attempt = InferenceTraceContext::enabled(
        Arc::clone(&writer),
        "root".into(),
        "turn".into(),
        "model".into(),
        "provider".into(),
    )
    .start_attempt();
    Ok((writer, attempt))
}

#[test]
fn inference_dispatch_can_finish_before_prepared_prompt_serialization() -> anyhow::Result<()> {
    struct Prompt {
        caller: thread::ThreadId,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl Serialize for Prompt {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            assert_ne!(thread::current().id(), self.caller);
            self.release.lock().unwrap().recv().unwrap();
            serializer.serialize_str("prepared")
        }
    }

    let dir = TempDir::new()?;
    let (writer, attempt) = recording(&dir)?;
    let (release, gate) = mpsc::channel();
    let wire = br#"{ "input": [] }"#;
    attempt.record_started(
        wire,
        Arc::new(Prompt {
            caller: thread::current().id(),
            release: Mutex::new(gate),
        }),
        || Ok(None),
    );
    attempt.record_completed("response", /*upstream_request_id*/ None, &None, &[]);
    release.send(())?;
    drop(attempt);
    drop(writer);
    let events = std::fs::read_to_string(dir.path().join("trace.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<RawTraceEvent>, _>>()?;
    let [start, end, closed] = &events[..] else {
        panic!("shutdown must publish both accepted events before recording closure")
    };
    assert!(matches!(
        closed.payload,
        RawTraceEventPayload::RecordingCompleted
    ));
    let RawTraceEventPayload::InferenceStarted {
        request_payload,
        prepared_prompt_payload,
        ..
    } = &start.payload
    else {
        panic!("start must precede completion")
    };
    assert!(matches!(
        end.payload,
        RawTraceEventPayload::InferenceCompleted { .. }
    ));
    let mut reader = PayloadReader::new(dir.path());
    assert_eq!(reader.read(request_payload)?, wire);
    assert_eq!(reader.read(prepared_prompt_payload)?, br#""prepared""#);
    Ok(())
}

#[test]
fn deferred_serialization_failure_suppresses_the_start_and_closing_marker() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let (writer, attempt) = recording(&dir)?;
    // JSON cannot encode an array as an object key.
    attempt.record_started(b"{}", Arc::new(BTreeMap::from([([1, 2], true)])), || {
        Ok(None)
    });
    assert!(writer.flush().is_err());
    drop(attempt);
    drop(writer);
    assert_eq!(std::fs::read_to_string(dir.path().join("trace.jsonl"))?, "");
    Ok(())
}

#[test]
fn borrowed_response_encoding_failure_preserves_the_queued_start() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let (writer, attempt) = recording(&dir)?;
    attempt.record_started(b"{}", Arc::new("prepared"), || Ok(None));
    // Handoff succeeds even though JSON cannot represent this response's map key.
    writer.event_with_payload(
        RawTraceEventContext::turn("root", "turn"),
        RawPayloadKind::InferenceResponse,
        &BTreeMap::from([([1, 2], true)]),
        |_| unreachable!("a failed payload has no reference to publish"),
    )?;
    assert!(
        writer
            .flush()
            .unwrap_err()
            .to_string()
            .contains("key must be a string")
    );
    drop(attempt);
    drop(writer);
    let log = std::fs::read_to_string(dir.path().join("trace.jsonl"))?;
    // Exactly one event survived. Parsing the whole log rejects a spurious closing marker.
    let event: RawTraceEvent = serde_json::from_str(&log)?;
    assert!(matches!(
        event.payload,
        RawTraceEventPayload::InferenceStarted { .. }
    ));
    Ok(())
}
