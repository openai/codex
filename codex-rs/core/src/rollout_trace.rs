//! Opt-in producer for the rollout trace bundle.
//!
//! This module is the deliberately thin bridge from `codex-core` into
//! `codex-rollout-trace`. Core emits raw observations; the trace crate's
//! offline reducer owns the semantic graph.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_rollout_trace::AgentThreadId;
use codex_rollout_trace::CompactionTraceContext;
use codex_rollout_trace::InferenceTraceContext;
use codex_rollout_trace::RawPayloadKind;
use codex_rollout_trace::RawTraceEventContext;
use codex_rollout_trace::RawTraceEventPayload;
use codex_rollout_trace::TraceWriter;
use serde::Serialize;
use tracing::debug;
use tracing::warn;
use uuid::Uuid;

/// Environment variable that enables local trace-bundle recording.
///
/// The value is a root directory. Each independent root session gets one child
/// bundle directory. Spawned child threads share their root session's bundle so
/// one reduced `state.json` describes the whole multi-agent rollout tree.
pub(crate) const CODEX_ROLLOUT_TRACE_ROOT_ENV: &str = "CODEX_ROLLOUT_TRACE_ROOT";

/// Lightweight handle stored in `SessionServices`.
///
/// Cloning the handle is cheap; all sequencing and file ownership remains
/// inside `TraceWriter`.
#[derive(Clone, Debug)]
pub(crate) struct RolloutTraceRecorder {
    writer: Arc<TraceWriter>,
}

/// Metadata captured once at thread/session start.
///
/// This payload is intentionally operational rather than reduced: it is a raw
/// payload that later reducers can mine as the reduced thread model evolves.
#[derive(Serialize)]
pub(crate) struct ThreadStartedTraceMetadata {
    pub(crate) thread_id: String,
    pub(crate) agent_path: String,
    pub(crate) task_name: Option<String>,
    pub(crate) nickname: Option<String>,
    pub(crate) agent_role: Option<String>,
    pub(crate) session_source: SessionSource,
    pub(crate) cwd: PathBuf,
    pub(crate) rollout_path: Option<PathBuf>,
    pub(crate) model: String,
    pub(crate) provider_name: String,
    pub(crate) approval_policy: String,
    pub(crate) sandbox_policy: String,
}

/// History replacement checkpoint persisted when compaction installs new live history.
///
/// The checkpoint keeps compaction separate from ordinary sampling snapshots:
/// `input_history` is the live thread history selected for compaction, while
/// `replacement_history` is what future prompts may carry after the checkpoint.
#[derive(Serialize)]
pub(crate) struct CompactionCheckpointTracePayload<'a> {
    pub(crate) input_history: &'a [ResponseItem],
    pub(crate) replacement_history: &'a [ResponseItem],
}

impl RolloutTraceRecorder {
    /// Creates and starts a trace bundle if `CODEX_ROLLOUT_TRACE_ROOT` is set.
    ///
    /// Trace startup is best-effort. A tracing failure must not make the Codex
    /// session unusable, because traces are diagnostic and can be enabled while
    /// debugging unrelated production failures.
    pub(crate) fn maybe_create(
        thread_id: ThreadId,
        metadata: ThreadStartedTraceMetadata,
    ) -> Option<Self> {
        let root = std::env::var_os(CODEX_ROLLOUT_TRACE_ROOT_ENV)?;
        let root = PathBuf::from(root);
        match Self::create_in_root(root.as_path(), thread_id, metadata) {
            Ok(recorder) => Some(recorder),
            Err(err) => {
                warn!("failed to initialize rollout trace recorder: {err:#}");
                None
            }
        }
    }

    fn create_in_root(
        root: &Path,
        thread_id: ThreadId,
        metadata: ThreadStartedTraceMetadata,
    ) -> anyhow::Result<Self> {
        let trace_id = Uuid::new_v4().to_string();
        let thread_id = thread_id.to_string();
        let bundle_dir = root.join(format!("trace-{trace_id}-{thread_id}"));
        let writer = TraceWriter::create(
            &bundle_dir,
            trace_id.clone(),
            thread_id.clone(),
            thread_id.clone(),
        )?;
        let recorder = Self {
            writer: Arc::new(writer),
        };

        recorder.append_best_effort(RawTraceEventPayload::RolloutStarted {
            trace_id,
            root_thread_id: thread_id,
        });

        recorder.record_thread_started(metadata);

        debug!("recording rollout trace at {}", bundle_dir.display());
        Ok(recorder)
    }

    /// Emits the lifecycle event and metadata for one thread in this rollout tree.
    ///
    /// Root sessions call this immediately after `RolloutStarted`; spawned
    /// child sessions call it on the inherited recorder. Keeping children in
    /// the root bundle preserves one raw payload namespace and one reduced
    /// `RolloutTrace` for the whole multi-agent task.
    pub(crate) fn record_thread_started(&self, metadata: ThreadStartedTraceMetadata) {
        let metadata_payload =
            self.write_json_payload_best_effort(RawPayloadKind::SessionMetadata, &metadata);
        self.append_best_effort(RawTraceEventPayload::ThreadStarted {
            thread_id: metadata.thread_id,
            agent_path: metadata.agent_path,
            metadata_payload,
        });
    }

    /// Builds reusable inference trace context for one Codex turn.
    ///
    /// The returned context is intentionally not "an inference call" yet.
    /// Transport code owns retry/fallback attempts and calls `start_attempt`
    /// only after it has built the concrete request payload for that attempt.
    pub(crate) fn inference_trace_context(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        model: String,
        provider_name: String,
    ) -> InferenceTraceContext {
        InferenceTraceContext::enabled(
            Arc::clone(&self.writer),
            thread_id,
            codex_turn_id,
            model,
            provider_name,
        )
    }

    /// Builds remote-compaction trace context for one checkpoint.
    ///
    /// Rollout tracing currently has a first-class checkpoint model only for remote compaction.
    /// The compact endpoint is a model-facing request whose output replaces live history, so it
    /// needs both request/response attempt events and a later checkpoint event when processed
    /// replacement history is installed.
    pub(crate) fn compaction_trace_context(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        compaction_id: String,
        model: String,
        provider_name: String,
    ) -> CompactionTraceContext {
        CompactionTraceContext::enabled(
            Arc::clone(&self.writer),
            thread_id,
            codex_turn_id,
            compaction_id,
            model,
            provider_name,
        )
    }

    /// Emits the checkpoint where remote-compacted history replaces live thread history.
    ///
    /// This checkpoint is deliberately separate from the compact endpoint response: Codex filters
    /// and reinjects context before replacement history becomes live. The reducer uses this event
    /// to connect the pre-compaction history to the processed replacement items without treating
    /// repeated developer/context prefix items as part of the replacement itself.
    pub(crate) fn record_compaction_installed(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        compaction_id: String,
        checkpoint: &CompactionCheckpointTracePayload<'_>,
    ) {
        let Some(checkpoint_payload) =
            self.write_json_payload_best_effort(RawPayloadKind::CompactionCheckpoint, checkpoint)
        else {
            return;
        };
        self.append_with_context_best_effort(
            thread_id,
            codex_turn_id,
            RawTraceEventPayload::CompactionInstalled {
                compaction_id,
                checkpoint_payload,
            },
        );
    }

    fn write_json_payload_best_effort(
        &self,
        kind: RawPayloadKind,
        payload: &impl Serialize,
    ) -> Option<codex_rollout_trace::RawPayloadRef> {
        match self.writer.write_json_payload(kind, payload) {
            Ok(payload_ref) => Some(payload_ref),
            Err(err) => {
                warn!("failed to write rollout trace payload: {err:#}");
                None
            }
        }
    }

    fn append_best_effort(&self, payload: RawTraceEventPayload) {
        if let Err(err) = self.writer.append(payload) {
            warn!("failed to append rollout trace event: {err:#}");
        }
    }

    fn append_with_context_best_effort(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        payload: RawTraceEventPayload,
    ) {
        let context = RawTraceEventContext {
            thread_id: Some(thread_id),
            codex_turn_id: Some(codex_turn_id),
        };
        if let Err(err) = self.writer.append_with_context(context, payload) {
            warn!("failed to append rollout trace event: {err:#}");
        }
    }
}

#[cfg(test)]
#[path = "rollout_trace_tests.rs"]
mod tests;
