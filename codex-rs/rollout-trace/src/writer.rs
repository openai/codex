//! Hot-path trace bundle writer.
//!
//! A single worker publishes live events in order while model and tool work proceeds.
//! Shutdown waits for the worker to drain accepted events and seal the bundle.

use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::mpsc;
use std::thread;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
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
use crate::shard::ShardWriter;

// Bound the number of retained prompts before their serialized sizes are known.
const MAX_PENDING_JOBS: usize = 8;
type WriteJob = Box<dyn FnOnce(&mut BundleWriter) + Send>;

/// Serializes trace writes in submission order. Explicit writes wait until their evidence is readable.
#[derive(Debug)]
pub struct TraceWriter {
    jobs: Option<mpsc::SyncSender<WriteJob>>,
    worker: Option<thread::JoinHandle<()>>,
    failure: Arc<OnceLock<String>>,
}

/// Owns the bundle files until shutdown. Each body is flushed before its event is published.
pub(crate) struct BundleWriter {
    rollout_id: String,
    events: BufWriter<File>,
    shards: ShardWriter,
    next_seq: u64,
    next_payload_ordinal: u64,
    // Keep the first error so explicit reads and shutdown can detect an incomplete recording.
    failure: Arc<OnceLock<String>>,
}

impl BundleWriter {
    fn record<T>(&mut self, write: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        check_recording(&self.failure)?;
        write(self).inspect_err(|error| {
            let error = format!("{error:#}");
            tracing::error!("rollout recording failed: {error}");
            let _ = self.failure.set(error);
        })
    }

    pub(crate) fn write_body(
        &mut self,
        kind: RawPayloadKind,
        body: &[u8],
    ) -> Result<RawPayloadRef> {
        let ordinal = self.next_payload_ordinal;
        let raw_payload_id = format!("raw_payload:{ordinal}");
        let shard = self.shards.append(&raw_payload_id, body)?;
        self.next_payload_ordinal += 1;
        Ok(RawPayloadRef {
            raw_payload_id,
            kind,
            shard: Some(shard),
            capture: crate::PayloadCapture::Complete,
            parts: Vec::new(),
        })
    }

    fn append(
        &mut self,
        context: RawTraceEventContext,
        payload: RawTraceEventPayload,
        mut wall_time_unix_ms: i64,
    ) -> Result<RawTraceEvent> {
        if matches!(payload, RawTraceEventPayload::RecordingCompleted) {
            self.shards.finish()?;
            // Closure is observed here, after draining earlier jobs.
            wall_time_unix_ms = unix_time_ms();
        }
        let event = RawTraceEvent {
            schema_version: RAW_TRACE_EVENT_SCHEMA_VERSION,
            seq: self.next_seq,
            wall_time_unix_ms,
            rollout_id: self.rollout_id.clone(),
            thread_id: context.thread_id,
            codex_turn_id: context.codex_turn_id,
            payload,
        };
        serde_json::to_writer(&mut self.events, &event)?;
        self.events.write_all(b"\n")?;
        self.events.flush()?;
        self.next_seq += 1;
        Ok(event)
    }
}

impl TraceWriter {
    /// Creates a trace bundle directory and writes its manifest.
    pub fn create(
        bundle_dir: impl AsRef<Path>,
        trace_id: String,
        rollout_id: String,
        root_thread_id: AgentThreadId,
    ) -> Result<Self> {
        let bundle_dir = bundle_dir.as_ref();
        let payloads_dir = bundle_dir.join(PAYLOADS_DIR_NAME);
        std::fs::create_dir_all(&payloads_dir)
            .with_context(|| format!("create trace payload dir {}", payloads_dir.display()))?;

        let started_at_unix_ms = unix_time_ms();
        let manifest =
            TraceBundleManifest::new(trace_id, rollout_id, root_thread_id, started_at_unix_ms);
        let event_log_path = bundle_dir.join(RAW_EVENT_LOG_FILE_NAME);
        let event_log = File::create_new(&event_log_path)
            .with_context(|| format!("open trace event log {}", event_log_path.display()))?;
        let manifest_path = bundle_dir.join(MANIFEST_FILE_NAME);
        std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)
            .with_context(|| format!("write trace manifest {}", manifest_path.display()))?;

        let failure = Arc::new(OnceLock::new());
        let mut bundle = BundleWriter {
            rollout_id: manifest.rollout_id,
            events: BufWriter::new(event_log),
            shards: ShardWriter::new(payloads_dir),
            next_seq: 1,
            next_payload_ordinal: 1,
            failure: Arc::clone(&failure),
        };
        let (jobs, receiver) = mpsc::sync_channel::<WriteJob>(MAX_PENDING_JOBS);
        let worker = thread::Builder::new()
            .name("rollout-trace".into())
            .spawn(move || {
                for job in receiver {
                    job(&mut bundle);
                }
            })
            .context("start rollout trace writer")?;
        Ok(Self {
            jobs: Some(jobs),
            worker: Some(worker),
            failure,
        })
    }

    fn submit(&self, job: impl FnOnce(&mut BundleWriter) + Send + 'static) -> Result<()> {
        check_recording(&self.failure)?;
        self.jobs
            .as_ref()
            .context("closed rollout trace writer")?
            .send(Box::new(job))
            .map_err(|_| anyhow!("rollout trace worker stopped"))
    }

    /// A caller that needs a published reference or a live read barrier waits in FIFO order.
    fn call<T: Send + 'static>(
        &self,
        write: impl FnOnce(&mut BundleWriter) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (reply, result) = mpsc::channel();
        self.submit(move |bundle| {
            let _ = reply.send(bundle.record(write));
        })?;
        result.recv().context("rollout trace worker stopped")?
    }

    /// Publishes a complete JSON body and returns its location in the bundle.
    pub fn write_json_payload(
        &self,
        kind: RawPayloadKind,
        value: &impl Serialize,
    ) -> Result<RawPayloadRef> {
        check_recording(&self.failure)?;
        let body = serde_json::to_vec(value);
        self.call(move |bundle| bundle.write_body(kind, &body?))
    }

    /// Appends one raw event with no extra envelope context.
    pub fn append(&self, payload: RawTraceEventPayload) -> Result<RawTraceEvent> {
        self.append_with_context(RawTraceEventContext::default(), payload)
    }

    /// Appends an event and waits for it to become readable.
    pub fn append_with_context(
        &self,
        context: RawTraceEventContext,
        payload: RawTraceEventPayload,
    ) -> Result<RawTraceEvent> {
        let now = unix_time_ms();
        self.call(move |bundle| bundle.append(context, payload, now))
    }

    /// The worker writes this event's bodies before publishing its references.
    pub(crate) fn defer_event(
        &self,
        context: RawTraceEventContext,
        build: impl FnOnce(&mut BundleWriter) -> Result<RawTraceEventPayload> + Send + 'static,
    ) -> Result<()> {
        let now = unix_time_ms();
        self.submit(move |bundle| {
            let _ = bundle.record(|bundle| {
                let payload = build(bundle)?;
                bundle.append(context, payload, now)
            });
        })
    }

    pub(crate) fn enqueue_event(
        &self,
        context: RawTraceEventContext,
        payload: RawTraceEventPayload,
    ) -> Result<()> {
        self.defer_event(context, move |_| Ok(payload))
    }

    /// Encodes a borrowed observation before returning. The worker handles its
    /// bytes or serialization error in order, preserving previously accepted evidence.
    pub(crate) fn event_with_payload(
        &self,
        context: RawTraceEventContext,
        kind: RawPayloadKind,
        value: &impl Serialize,
        event: impl FnOnce(RawPayloadRef) -> RawTraceEventPayload + Send + 'static,
    ) -> Result<()> {
        check_recording(&self.failure)?;
        let body = serde_json::to_vec(value);
        self.defer_event(context, move |bundle| {
            let reference = bundle.write_body(kind, &body?)?;
            Ok(event(reference))
        })
    }

    /// Waits for earlier records to become readable, or returns the first recording failure.
    pub fn flush(&self) -> Result<()> {
        self.call(|_| Ok(()))
    }
}

impl Drop for TraceWriter {
    fn drop(&mut self) {
        // The closing event seals the shard after earlier jobs. Joining makes it visible on return.
        if !thread::panicking() {
            let _ = self.enqueue_event(
                RawTraceEventContext::default(),
                RawTraceEventPayload::RecordingCompleted,
            );
        }
        drop(self.jobs.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn check_recording(failure: &OnceLock<String>) -> Result<()> {
    if let Some(error) = failure.get() {
        bail!("rollout recording failed: {error}");
    }
    Ok(())
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
            prepared_prompt_payload: writer
                .write_json_payload(RawPayloadKind::PreparedPrompt, &json!({"input": []}))?,
            warmup_context_payload: None,
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
            rollout.raw_payloads[&metadata_payload.raw_payload_id].shard,
            Some(1)
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
        assert!(writer.flush().is_err());
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
        let file = std::fs::File::open(temp.path().join("trace.jsonl"))?;
        writer.call(move |bundle| {
            bundle.events = std::io::BufWriter::new(file);
            Ok(())
        })?;
        assert!(
            writer
                .append(RawTraceEventPayload::RolloutEnded {
                    status: RolloutStatus::Completed
                })
                .is_err()
        );
        assert!(writer.flush().is_err());
        drop(writer);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("trace.jsonl"))?,
            ""
        );
        Ok(())
    }
}
