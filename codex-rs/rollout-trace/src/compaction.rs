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

use crate::inference::serialize_trace_items;
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
struct TracedCompactionCompleted<'a> {
    #[serde(serialize_with = "serialize_trace_items")]
    output_items: &'a [ResponseItem],
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

    /// Returns whether this context records compaction traces.
    pub fn is_enabled(&self) -> bool {
        matches!(self.state, CompactionTraceContextState::Enabled(_))
    }

    /// Starts an upstream attempt. The recording worker builds the snapshot if tracing is enabled.
    pub fn start_attempt<T: Serialize>(
        &self,
        request: impl FnOnce() -> T + Send + 'static,
    ) -> CompactionTraceAttempt {
        let CompactionTraceContextState::Enabled(context) = &self.state else {
            return CompactionTraceAttempt {
                state: CompactionTraceAttemptState::Disabled,
            };
        };

        let compaction_request_id = next_compaction_request_id();
        let attempt = CompactionTraceAttempt {
            state: CompactionTraceAttemptState::Enabled(EnabledCompactionTraceAttempt {
                context: context.clone(),
                compaction_request_id: compaction_request_id.clone(),
            }),
        };
        let compaction_id = context.compaction_id.clone();
        let thread_id = context.thread_id.clone();
        let codex_turn_id = context.codex_turn_id.clone();
        let model = context.model.clone();
        let provider_name = context.provider_name.clone();
        let _ = context.writer.defer_event(
            RawTraceEventContext::turn(&thread_id, &codex_turn_id),
            move |bundle| {
                let request_payload = bundle.write_body(
                    RawPayloadKind::CompactionRequest,
                    &serde_json::to_vec(&request())?,
                )?;
                Ok(RawTraceEventPayload::CompactionRequestStarted {
                    compaction_id,
                    compaction_request_id,
                    thread_id,
                    codex_turn_id,
                    model,
                    provider_name,
                    request_payload,
                })
            },
        );
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
        let compaction_id = context.compaction_id.clone();
        let _ = context.writer.event_with_payload(
            RawTraceEventContext::turn(&context.thread_id, &context.codex_turn_id),
            RawPayloadKind::CompactionCheckpoint,
            checkpoint,
            move |checkpoint_payload| RawTraceEventPayload::CompactionInstalled {
                compaction_id,
                checkpoint_payload,
            },
        );
    }
}

impl CompactionTraceAttempt {
    /// Records the non-streaming compact endpoint response payload.
    ///
    /// Compaction responses use the same response-item preservation rules as
    /// inference streams: traces are evidence, while normal ResponseItem
    /// serialization is shaped for future request construction.
    pub fn record_completed(&self, output_items: &[ResponseItem]) {
        let CompactionTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        let response_payload = TracedCompactionCompleted { output_items };
        let ctx = &attempt.context;
        let compaction_id = ctx.compaction_id.clone();
        let compaction_request_id = attempt.compaction_request_id.clone();
        let _ = ctx.writer.event_with_payload(
            RawTraceEventContext::turn(&ctx.thread_id, &ctx.codex_turn_id),
            RawPayloadKind::CompactionResponse,
            &response_payload,
            move |response_payload| RawTraceEventPayload::CompactionRequestCompleted {
                compaction_id,
                compaction_request_id,
                response_payload,
            },
        );
    }

    /// Records the compact endpoint result without forcing callers to branch on trace events.
    pub fn record_result<E: Display>(&self, result: Result<&[ResponseItem], E>) {
        match result {
            Ok(output_items) => self.record_completed(output_items),
            Err(err) => self.record_failed(err),
        }
    }

    /// Records pre-response failures from the compact endpoint.
    pub fn record_failed(&self, error: impl Display) {
        let CompactionTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        let _ = attempt.context.writer.enqueue_event(
            RawTraceEventContext::turn(&attempt.context.thread_id, &attempt.context.codex_turn_id),
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
