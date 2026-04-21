//! Hot-path helpers for recording upstream remote compaction attempts.
//!
//! Remote compaction is a model-facing request with a different semantic role
//! from normal sampling. Keeping the no-op capable trace handle in this crate
//! lets `codex-core` record exact endpoint payloads without owning trace schema
//! details.

use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_protocol::models::ResponseItem;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tracing::warn;

use crate::inference::trace_response_item_json;
use crate::model::AgentThreadId;
use crate::model::CodexTurnId;
use crate::model::CompactionId;
use crate::model::CompactionRequestId;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;
use crate::writer::TraceWriter;

static NEXT_COMPACTION_REQUEST: AtomicU64 = AtomicU64::new(1);

/// Turn-local remote compaction tracing context.
///
/// A compaction can retry its upstream request before installing one checkpoint. The context
/// owns the stable checkpoint ID; each request attempt gets a separate request ID.
#[derive(Clone, Debug)]
pub struct CompactionTraceContext {
    state: CompactionTraceContextState,
}

#[derive(Clone, Debug)]
enum CompactionTraceContextState {
    Disabled,
    Enabled(EnabledCompactionTraceContext),
}

#[derive(Clone, Debug)]
struct EnabledCompactionTraceContext {
    writer: Arc<TraceWriter>,
    thread_id: AgentThreadId,
    codex_turn_id: CodexTurnId,
    compaction_id: CompactionId,
    model: String,
    provider_name: String,
}

/// One upstream request attempt made while computing a compaction checkpoint.
#[derive(Clone, Debug)]
pub struct CompactionTraceAttempt {
    state: CompactionTraceAttemptState,
}

#[derive(Clone, Debug)]
enum CompactionTraceAttemptState {
    Disabled,
    Enabled(EnabledCompactionTraceAttempt),
}

#[derive(Clone, Debug)]
struct EnabledCompactionTraceAttempt {
    context: EnabledCompactionTraceContext,
    compaction_request_id: CompactionRequestId,
}

#[derive(Serialize)]
struct TracedCompactionCompleted {
    output_items: Vec<JsonValue>,
}

/// History replacement checkpoint persisted when compaction installs new live history.
///
/// The checkpoint keeps compaction separate from ordinary sampling snapshots:
/// `input_history` is the live thread history selected for compaction, while
/// `replacement_history` is what future prompts may carry after the checkpoint.
#[derive(Serialize)]
pub struct CompactionCheckpointTracePayload<'a> {
    pub input_history: &'a [ResponseItem],
    pub replacement_history: &'a [ResponseItem],
}

impl CompactionTraceContext {
    /// Builds a context that accepts trace calls and records nothing.
    pub fn disabled() -> Self {
        Self {
            state: CompactionTraceContextState::Disabled,
        }
    }

    /// Builds an enabled context for upstream attempts that compute one checkpoint.
    pub fn enabled(
        writer: Arc<TraceWriter>,
        thread_id: AgentThreadId,
        codex_turn_id: CodexTurnId,
        compaction_id: CompactionId,
        model: String,
        provider_name: String,
    ) -> Self {
        Self {
            state: CompactionTraceContextState::Enabled(EnabledCompactionTraceContext {
                writer,
                thread_id,
                codex_turn_id,
                compaction_id,
                model,
                provider_name,
            }),
        }
    }

    /// Starts a new upstream attempt and records the exact compact endpoint request.
    pub fn start_attempt(&self, request: &impl Serialize) -> CompactionTraceAttempt {
        let CompactionTraceContextState::Enabled(context) = &self.state else {
            return CompactionTraceAttempt::disabled();
        };

        let attempt = CompactionTraceAttempt {
            state: CompactionTraceAttemptState::Enabled(EnabledCompactionTraceAttempt {
                context: context.clone(),
                compaction_request_id: next_compaction_request_id(),
            }),
        };
        attempt.record_started(request);
        attempt
    }

    /// Records the point where compacted history becomes the live thread history.
    ///
    /// The checkpoint belongs to the same semantic compaction lifecycle as the
    /// compact endpoint attempts, so the context reuses its stable compaction ID.
    pub fn record_installed(&self, checkpoint: &CompactionCheckpointTracePayload<'_>) {
        let CompactionTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        let checkpoint_payload = match context
            .writer
            .write_json_payload(RawPayloadKind::CompactionCheckpoint, checkpoint)
        {
            Ok(payload_ref) => payload_ref,
            Err(err) => {
                warn!("failed to write rollout trace payload: {err:#}");
                return;
            }
        };

        let event_context = RawTraceEventContext {
            thread_id: Some(context.thread_id.clone()),
            codex_turn_id: Some(context.codex_turn_id.clone()),
        };
        if let Err(err) = context.writer.append_with_context(
            event_context,
            RawTraceEventPayload::CompactionInstalled {
                compaction_id: context.compaction_id.clone(),
                checkpoint_payload,
            },
        ) {
            warn!("failed to append rollout trace event: {err:#}");
        }
    }
}

impl CompactionTraceAttempt {
    /// Builds an attempt that records nothing.
    fn disabled() -> Self {
        Self {
            state: CompactionTraceAttemptState::Disabled,
        }
    }

    fn record_started(&self, request: &impl Serialize) {
        let CompactionTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        let Some(request_payload) = write_json_payload_best_effort(
            &attempt.context.writer,
            RawPayloadKind::CompactionRequest,
            request,
        ) else {
            return;
        };

        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::CompactionRequestStarted {
                compaction_id: attempt.context.compaction_id.clone(),
                compaction_request_id: attempt.compaction_request_id.clone(),
                thread_id: attempt.context.thread_id.clone(),
                codex_turn_id: attempt.context.codex_turn_id.clone(),
                model: attempt.context.model.clone(),
                provider_name: attempt.context.provider_name.clone(),
                request_payload,
            },
        );
    }

    /// Records the non-streaming compact endpoint response payload.
    ///
    /// Compaction responses use the same response-item preservation rules as
    /// inference streams: traces are evidence, while normal ResponseItem
    /// serialization is shaped for future request construction.
    pub fn record_completed(&self, output_items: &[ResponseItem]) {
        let response_payload = TracedCompactionCompleted {
            output_items: output_items.iter().map(trace_response_item_json).collect(),
        };
        let CompactionTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        let Some(response_payload) = write_json_payload_best_effort(
            &attempt.context.writer,
            RawPayloadKind::CompactionResponse,
            &response_payload,
        ) else {
            return;
        };

        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::CompactionRequestCompleted {
                compaction_id: attempt.context.compaction_id.clone(),
                compaction_request_id: attempt.compaction_request_id.clone(),
                response_payload,
            },
        );
    }

    /// Records pre-response failures from the compact endpoint.
    pub fn record_failed(&self, error: impl Display) {
        let CompactionTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::CompactionRequestFailed {
                compaction_id: attempt.context.compaction_id.clone(),
                compaction_request_id: attempt.compaction_request_id.clone(),
                error: error.to_string(),
            },
        );
    }
}

fn next_compaction_request_id() -> CompactionRequestId {
    let ordinal = NEXT_COMPACTION_REQUEST.fetch_add(1, Ordering::Relaxed);
    format!("compaction_request:{ordinal}")
}

fn write_json_payload_best_effort(
    writer: &TraceWriter,
    kind: RawPayloadKind,
    payload: &impl Serialize,
) -> Option<crate::RawPayloadRef> {
    writer.write_json_payload(kind, payload).ok()
}

fn append_with_context_best_effort(
    context: &EnabledCompactionTraceContext,
    payload: RawTraceEventPayload,
) {
    let event_context = RawTraceEventContext {
        thread_id: Some(context.thread_id.clone()),
        codex_turn_id: Some(context.codex_turn_id.clone()),
    };
    let _ = context.writer.append_with_context(event_context, payload);
}
