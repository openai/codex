//! Hot-path trace bundle writer.
//!
//! All producers share one writer. Write failures prevent further recording and
//! suppress the final closing event.

use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Serialize;

use crate::bundle::MANIFEST_FILE_NAME;
use crate::bundle::PAYLOADS_DIR_NAME;
use crate::bundle::RAW_EVENT_LOG_FILE_NAME;
use crate::bundle::TraceBundleManifest;
use crate::model::AgentThreadId;
use crate::payload::RawPayloadKind;
use crate::payload::RawPayloadRef;
use crate::raw_event::RAW_TRACE_EVENT_SCHEMA_VERSION;
use crate::raw_event::RawTraceEvent;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;

/// Local trace bundle writer.
///
/// The writer appends raw events and writes payload files. It does not keep a
/// reduced `RolloutTrace` in memory; replay is owned by the reducer.
#[derive(Debug)]
pub struct TraceWriter {
    inner: Mutex<TraceWriterInner>,
}

#[derive(Debug)]
struct TraceWriterInner {
    manifest: TraceBundleManifest,
    payloads_dir: PathBuf,
    event_log: BufWriter<File>,
    next_seq: u64,
    next_payload_ordinal: u64,
    // Preserve the first failure even when a producer cannot return it to its caller.
    error: Option<String>,
}

impl TraceWriter {
    /// Creates a trace bundle directory and writes its manifest.
    pub fn create(
        bundle_dir: impl AsRef<Path>,
        trace_id: String,
        rollout_id: String,
        root_thread_id: AgentThreadId,
    ) -> Result<Self> {
        let bundle_dir = bundle_dir.as_ref().to_path_buf();
        let payloads_dir = bundle_dir.join(PAYLOADS_DIR_NAME);
        std::fs::create_dir_all(&payloads_dir)
            .with_context(|| format!("create trace payload dir {}", payloads_dir.display()))?;

        let started_at_unix_ms = unix_time_ms();
        let manifest =
            TraceBundleManifest::new(trace_id, rollout_id, root_thread_id, started_at_unix_ms);
        let event_log_path = bundle_dir.join(RAW_EVENT_LOG_FILE_NAME);
        let event_log = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&event_log_path)
            .with_context(|| format!("open trace event log {}", event_log_path.display()))?;
        write_json_file(&bundle_dir.join(MANIFEST_FILE_NAME), &manifest)?;

        Ok(Self {
            inner: Mutex::new(TraceWriterInner {
                manifest,
                payloads_dir,
                event_log: BufWriter::new(event_log),
                next_seq: 1,
                next_payload_ordinal: 1,
                error: None,
            }),
        })
    }

    /// Writes a JSON payload file and returns its reduced-state reference.
    pub fn write_json_payload(
        &self,
        kind: RawPayloadKind,
        value: &impl Serialize,
    ) -> Result<RawPayloadRef> {
        let mut inner = self.lock_inner();
        inner.check()?;
        let ordinal = inner.next_payload_ordinal;
        inner.next_payload_ordinal += 1;
        let raw_payload_id = format!("raw_payload:{ordinal}");
        let relative_path = format!("{PAYLOADS_DIR_NAME}/{ordinal}.json");
        let absolute_path = inner.payloads_dir.join(format!("{ordinal}.json"));
        // Payload files are created before the event that references them. A
        // replay interrupted after an event is appended should never point at a
        // payload file that the writer planned but had not written yet.
        let result = write_json_file(&absolute_path, value);
        inner.record_result(result)?;
        Ok(RawPayloadRef {
            raw_payload_id,
            kind,
            path: relative_path,
        })
    }

    /// Appends one raw event with no extra envelope context.
    pub fn append(&self, payload: RawTraceEventPayload) -> Result<RawTraceEvent> {
        self.append_with_context(RawTraceEventContext::default(), payload)
    }

    /// Appends one raw event with explicit thread/turn context.
    pub fn append_with_context(
        &self,
        context: RawTraceEventContext,
        payload: RawTraceEventPayload,
    ) -> Result<RawTraceEvent> {
        let mut inner = self.lock_inner();
        inner.check()?;
        let event = RawTraceEvent {
            schema_version: RAW_TRACE_EVENT_SCHEMA_VERSION,
            seq: inner.next_seq,
            wall_time_unix_ms: unix_time_ms(),
            rollout_id: inner.manifest.rollout_id.clone(),
            thread_id: context.thread_id,
            codex_turn_id: context.codex_turn_id,
            payload,
        };
        let result = (|| {
            serde_json::to_writer(&mut inner.event_log, &event)?;
            inner.event_log.write_all(b"\n")?;
            inner.event_log.flush()?;
            Ok(())
        })();
        inner.record_result(result)?;
        inner.next_seq += 1;
        Ok(event)
    }

    /// Reports any earlier recording failure, including failures in detached producers.
    pub(crate) fn check(&self) -> Result<()> {
        self.lock_inner().check()
    }

    fn lock_inner(&self) -> MutexGuard<'_, TraceWriterInner> {
        // Preserve the event log after a panic in tracing code. Dropping the
        // writer would lose subsequent diagnostic events in exactly the session
        // we are trying to debug.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl TraceWriterInner {
    fn check(&self) -> Result<()> {
        if let Some(error) = &self.error {
            bail!("rollout recording failed: {error}");
        }
        Ok(())
    }

    fn record_result<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            let error = format!("{error:#}");
            tracing::error!("rollout recording failed: {error}");
            self.error = Some(error);
        }
        result
    }
}

impl Drop for TraceWriter {
    fn drop(&mut self) {
        // The last producer releases the writer, even for interrupted turns.
        // This marks recorder closure, not successful task completion.
        if !self.inner.is_poisoned() && !std::thread::panicking() {
            let _ = self.append(RawTraceEventPayload::RecordingCompleted);
        }
    }
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)
        .with_context(|| format!("write JSON {}", path.display()))?;
    writer
        .flush()
        .with_context(|| format!("flush JSON {}", path.display()))
}

pub(crate) fn unix_time_ms() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

    use crate::model::ExecutionStatus;
    use crate::model::RolloutStatus;
    use crate::payload::RawPayloadKind;
    use crate::raw_event::RawTraceEventPayload;
    use crate::replay_bundle;
    use crate::writer::TraceWriter;

    #[test]
    fn writer_records_payload_refs_and_replays_rollout_status() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let writer = TraceWriter::create(
            temp.path(),
            "trace-1".to_string(),
            "rollout-1".to_string(),
            "thread-root".to_string(),
        )?;

        writer.append(RawTraceEventPayload::RolloutStarted {
            trace_id: "trace-1".to_string(),
            root_thread_id: "thread-root".to_string(),
        })?;
        let metadata_payload = writer.write_json_payload(
            RawPayloadKind::ProtocolEvent,
            &json!({
                "source": "test",
                "model": "gpt-test",
            }),
        )?;
        writer.append(RawTraceEventPayload::ThreadStarted {
            thread_id: "thread-root".to_string(),
            agent_path: "/root".to_string(),
            metadata_payload: Some(metadata_payload.clone()),
        })?;
        writer.append(RawTraceEventPayload::CodexTurnStarted {
            codex_turn_id: "turn-1".to_string(),
            thread_id: "thread-root".to_string(),
        })?;
        let inference_request = writer.write_json_payload(
            RawPayloadKind::InferenceRequest,
            &json!({
                "model": "gpt-test",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "hello"}]
                }],
            }),
        )?;
        writer.append(RawTraceEventPayload::InferenceStarted {
            inference_call_id: "inference-1".to_string(),
            thread_id: "thread-root".to_string(),
            codex_turn_id: "turn-1".to_string(),
            model: "gpt-test".to_string(),
            provider_name: "test-provider".to_string(),
            request_payload: inference_request.clone(),
        })?;
        let inference_response = writer.write_json_payload(
            RawPayloadKind::InferenceResponse,
            &json!({
                "response_id": "resp-1",
                "output_items": [],
            }),
        )?;
        writer.append(RawTraceEventPayload::InferenceCompleted {
            inference_call_id: "inference-1".to_string(),
            response_id: Some("resp-1".to_string()),
            upstream_request_id: Some("req-1".to_string()),
            response_payload: inference_response.clone(),
        })?;
        writer.append(RawTraceEventPayload::CodexTurnEnded {
            codex_turn_id: "turn-1".to_string(),
            status: ExecutionStatus::Completed,
        })?;
        writer.append(RawTraceEventPayload::RolloutEnded {
            status: RolloutStatus::Completed,
        })?;

        let rollout = replay_bundle(temp.path())?;

        assert_eq!(rollout.status, RolloutStatus::Completed);
        assert_eq!(rollout.root_thread_id, "thread-root");
        assert_eq!(rollout.threads["thread-root"].agent_path, "/root");
        assert_eq!(rollout.codex_turns["turn-1"].thread_id, "thread-root");
        assert_eq!(
            rollout.codex_turns["turn-1"].execution.status,
            ExecutionStatus::Completed,
        );
        assert_eq!(
            rollout.inference_calls["inference-1"].raw_request_payload_id,
            inference_request.raw_payload_id,
        );
        assert_eq!(
            rollout.inference_calls["inference-1"].raw_response_payload_id,
            Some(inference_response.raw_payload_id),
        );
        assert_eq!(
            rollout.raw_payloads[&metadata_payload.raw_payload_id].path,
            "payloads/1.json"
        );

        Ok(())
    }

    #[test]
    fn payload_failure_prevents_finalization_even_after_storage_recovers() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let writer =
            TraceWriter::create(temp.path(), "trace".into(), "rollout".into(), "root".into())?;
        std::fs::remove_dir(temp.path().join("payloads"))?;
        assert!(
            writer
                .write_json_payload(RawPayloadKind::ProtocolEvent, &json!({}))
                .is_err()
        );
        std::fs::create_dir(temp.path().join("payloads"))?;
        assert!(writer.check().is_err());
        assert!(
            writer
                .write_json_payload(RawPayloadKind::ProtocolEvent, &json!({}))
                .is_err()
        );
        drop(writer);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("trace.jsonl"))?,
            ""
        );
        Ok(())
    }

    #[test]
    fn event_write_failure_prevents_finalization() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let writer =
            TraceWriter::create(temp.path(), "trace".into(), "rollout".into(), "root".into())?;
        // A read-only file makes the real buffered write fail on every platform.
        writer.lock_inner().event_log =
            std::io::BufWriter::new(std::fs::File::open(temp.path().join("trace.jsonl"))?);
        assert!(
            writer
                .append(RawTraceEventPayload::RolloutEnded {
                    status: RolloutStatus::Completed
                })
                .is_err()
        );
        assert!(writer.check().is_err());
        drop(writer);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("trace.jsonl"))?,
            ""
        );
        Ok(())
    }
}
