//! Opt-in hot-path producer for rollout trace bundles.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use serde::Serialize;
use tracing::debug;
use tracing::warn;
use uuid::Uuid;

use crate::AgentThreadId;
use crate::CodeCellTraceContext;
use crate::CodexTurnId;
use crate::CompactionId;
use crate::CompactionTraceContext;
use crate::InferenceTraceContext;
use crate::RawPayloadKind;
use crate::RawPayloadRef;
use crate::RawTraceEventPayload;
use crate::ToolDispatchInvocation;
use crate::ToolDispatchTraceContext;
use crate::TraceWriter;

/// Environment variable that enables local trace-bundle recording.
///
/// The value is a root directory. Each independent root session gets one child
/// bundle directory. Spawned child threads share their root session's bundle so
/// one reduced `state.json` describes the whole multi-agent rollout tree.
pub const CODEX_ROLLOUT_TRACE_ROOT_ENV: &str = "CODEX_ROLLOUT_TRACE_ROOT";

/// Lightweight handle stored in `SessionServices`.
///
/// Cloning the handle is cheap; all sequencing and file ownership remains
/// inside `TraceWriter`. Disabled handles intentionally accept the same calls
/// as enabled handles so hot-path session code can describe traceable events
/// without repeatedly branching on whether diagnostic recording is enabled.
#[derive(Clone, Debug)]
pub struct RolloutTraceRecorder {
    state: RolloutTraceRecorderState,
}

#[derive(Clone, Debug)]
enum RolloutTraceRecorderState {
    Disabled,
    Enabled(EnabledRolloutTraceRecorder),
}

#[derive(Clone, Debug)]
struct EnabledRolloutTraceRecorder {
    writer: Arc<TraceWriter>,
}

/// Metadata captured once at thread/session start.
///
/// This payload is intentionally operational rather than reduced: it is a raw
/// payload that later reducers can mine as the reduced thread model evolves.
#[derive(Serialize)]
pub struct ThreadStartedTraceMetadata {
    pub thread_id: String,
    pub agent_path: String,
    pub task_name: Option<String>,
    pub nickname: Option<String>,
    pub agent_role: Option<String>,
    pub session_source: SessionSource,
    pub cwd: PathBuf,
    pub rollout_path: Option<PathBuf>,
    pub model: String,
    pub provider_name: String,
    pub approval_policy: String,
    pub sandbox_policy: String,
}

impl RolloutTraceRecorder {
    /// Builds a recorder handle that accepts trace calls and records nothing.
    pub fn disabled() -> Self {
        Self {
            state: RolloutTraceRecorderState::Disabled,
        }
    }

    /// Creates and starts a root trace bundle, or returns a disabled recorder.
    ///
    /// Trace startup is best-effort. A tracing failure must not make the Codex
    /// session unusable, because traces are diagnostic and can be enabled while
    /// debugging unrelated production failures. The returned recorder has not
    /// emitted `ThreadStarted`; session setup records that event uniformly for
    /// root and inherited child recorders.
    pub fn create_root_or_disabled(thread_id: ThreadId) -> Self {
        let Some(root) = std::env::var_os(CODEX_ROLLOUT_TRACE_ROOT_ENV) else {
            return Self::disabled();
        };
        let root = PathBuf::from(root);
        match Self::create_in_root(root.as_path(), thread_id) {
            Ok(recorder) => recorder,
            Err(err) => {
                warn!("failed to initialize rollout trace recorder: {err:#}");
                Self::disabled()
            }
        }
    }

    /// Creates a trace bundle in a known root directory.
    ///
    /// This is public so integration tests in downstream crates can replay the
    /// exact bundle they produced without mutating process environment.
    pub fn create_in_root_for_test(root: &Path, thread_id: ThreadId) -> anyhow::Result<Self> {
        Self::create_in_root(root, thread_id)
    }

    fn create_in_root(root: &Path, thread_id: ThreadId) -> anyhow::Result<Self> {
        let trace_id = Uuid::new_v4().to_string();
        let thread_id = thread_id.to_string();
        let bundle_dir = root.join(format!("trace-{trace_id}-{thread_id}"));
        let writer = TraceWriter::create(
            &bundle_dir,
            trace_id.clone(),
            thread_id.clone(),
            thread_id.clone(),
        )?;
        let recorder = EnabledRolloutTraceRecorder {
            writer: Arc::new(writer),
        };

        recorder.append_best_effort(RawTraceEventPayload::RolloutStarted {
            trace_id,
            root_thread_id: thread_id,
        });

        debug!("recording rollout trace at {}", bundle_dir.display());
        Ok(Self::enabled(recorder))
    }

    fn enabled(inner: EnabledRolloutTraceRecorder) -> Self {
        Self {
            state: RolloutTraceRecorderState::Enabled(inner),
        }
    }

    /// Emits the lifecycle event and metadata for one thread in this rollout tree.
    ///
    /// Root sessions call this immediately after `RolloutStarted`; spawned
    /// child sessions call it on the inherited recorder. Keeping children in
    /// the root bundle preserves one raw payload namespace and one reduced
    /// `RolloutTrace` for the whole multi-agent task.
    pub fn record_thread_started(&self, metadata: ThreadStartedTraceMetadata) {
        let RolloutTraceRecorderState::Enabled(recorder) = &self.state else {
            return;
        };
        let metadata_payload =
            recorder.write_json_payload_best_effort(RawPayloadKind::SessionMetadata, &metadata);
        recorder.append_best_effort(RawTraceEventPayload::ThreadStarted {
            thread_id: metadata.thread_id,
            agent_path: metadata.agent_path,
            metadata_payload,
        });
    }

    /// Emits a turn-start lifecycle event.
    ///
    /// Most production turn lifecycle wiring lives outside this PR layer, but
    /// trace-focused integration tests need a small explicit hook so reducer
    /// inputs remain valid without exercising the full session loop.
    pub fn record_codex_turn_started(
        &self,
        thread_id: impl Into<AgentThreadId>,
        codex_turn_id: impl Into<CodexTurnId>,
    ) {
        let RolloutTraceRecorderState::Enabled(recorder) = &self.state else {
            return;
        };
        let thread_id = thread_id.into();
        let codex_turn_id = codex_turn_id.into();
        recorder.append_with_context_best_effort(
            thread_id.clone(),
            codex_turn_id.clone(),
            RawTraceEventPayload::CodexTurnStarted {
                codex_turn_id,
                thread_id,
            },
        );
    }

    /// Starts a first-class code-mode cell lifecycle and returns its trace handle.
    pub fn start_code_cell_trace(
        &self,
        thread_id: impl Into<AgentThreadId>,
        codex_turn_id: impl Into<CodexTurnId>,
        runtime_cell_id: impl Into<String>,
        model_visible_call_id: impl Into<String>,
        source_js: impl Into<String>,
    ) -> CodeCellTraceContext {
        let context = self.code_cell_trace_context(thread_id, codex_turn_id, runtime_cell_id);
        context.record_started(model_visible_call_id, source_js);
        context
    }

    /// Builds a trace handle for an already-started code-mode runtime cell.
    pub fn code_cell_trace_context(
        &self,
        thread_id: impl Into<AgentThreadId>,
        codex_turn_id: impl Into<CodexTurnId>,
        runtime_cell_id: impl Into<String>,
    ) -> CodeCellTraceContext {
        let RolloutTraceRecorderState::Enabled(recorder) = &self.state else {
            return CodeCellTraceContext::disabled();
        };

        CodeCellTraceContext::enabled(
            Arc::clone(&recorder.writer),
            thread_id,
            codex_turn_id,
            runtime_cell_id,
        )
    }

    /// Starts one dispatch-level tool lifecycle and returns its trace handle.
    ///
    /// `invocation` is lazy because adapting core tool objects into trace-owned
    /// payloads can clone large arguments. Disabled tracing should not pay that
    /// cost on the hot tool-dispatch path.
    pub fn start_tool_dispatch_trace(
        &self,
        invocation: impl FnOnce() -> Option<ToolDispatchInvocation>,
    ) -> ToolDispatchTraceContext {
        let RolloutTraceRecorderState::Enabled(recorder) = &self.state else {
            return ToolDispatchTraceContext::disabled();
        };
        let Some(invocation) = invocation() else {
            return ToolDispatchTraceContext::disabled();
        };

        ToolDispatchTraceContext::start(Arc::clone(&recorder.writer), invocation)
    }

    /// Builds reusable inference trace context for one Codex turn.
    ///
    /// The returned context is intentionally not "an inference call" yet.
    /// Transport code owns retry/fallback attempts and calls `start_attempt`
    /// only after it has built the concrete request payload for that attempt.
    pub fn inference_trace_context(
        &self,
        thread_id: impl Into<AgentThreadId>,
        codex_turn_id: impl Into<CodexTurnId>,
        model: impl Into<String>,
        provider_name: impl Into<String>,
    ) -> InferenceTraceContext {
        let RolloutTraceRecorderState::Enabled(recorder) = &self.state else {
            return InferenceTraceContext::disabled();
        };

        InferenceTraceContext::enabled(
            Arc::clone(&recorder.writer),
            thread_id.into(),
            codex_turn_id.into(),
            model.into(),
            provider_name.into(),
        )
    }

    /// Builds remote-compaction trace context for one checkpoint.
    ///
    /// Rollout tracing currently has a first-class checkpoint model only for remote compaction.
    /// The compact endpoint is a model-facing request whose output replaces live history, so it
    /// needs both request/response attempt events and a later checkpoint event when processed
    /// replacement history is installed.
    pub fn compaction_trace_context(
        &self,
        thread_id: impl Into<AgentThreadId>,
        codex_turn_id: impl Into<CodexTurnId>,
        compaction_id: impl Into<CompactionId>,
        model: impl Into<String>,
        provider_name: impl Into<String>,
    ) -> CompactionTraceContext {
        let RolloutTraceRecorderState::Enabled(recorder) = &self.state else {
            return CompactionTraceContext::disabled();
        };

        CompactionTraceContext::enabled(
            Arc::clone(&recorder.writer),
            thread_id.into(),
            codex_turn_id.into(),
            compaction_id.into(),
            model.into(),
            provider_name.into(),
        )
    }
}

impl EnabledRolloutTraceRecorder {
    fn write_json_payload_best_effort(
        &self,
        kind: RawPayloadKind,
        payload: &impl Serialize,
    ) -> Option<RawPayloadRef> {
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
        codex_turn_id: CodexTurnId,
        payload: RawTraceEventPayload,
    ) {
        let context = crate::RawTraceEventContext {
            thread_id: Some(thread_id),
            codex_turn_id: Some(codex_turn_id),
        };
        if let Err(err) = self.writer.append_with_context(context, payload) {
            warn!("failed to append rollout trace event: {err:#}");
        }
    }
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
