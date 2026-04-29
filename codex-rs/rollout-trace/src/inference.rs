//! Hot-path helpers for recording upstream inference attempts.
//!
//! The model client should not need to know whether rollout tracing is enabled.
//! A disabled context records nothing, which keeps one-shot HTTP calls,
//! WebSocket reuse, and retry/fallback attempts on the same code path.

use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::model::AgentThreadId;
use crate::model::CodexTurnId;
use crate::model::InferenceCallId;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;
use crate::writer::TraceWriter;

static NEXT_INFERENCE_ATTEMPT: AtomicU64 = AtomicU64::new(1);

/// Turn-local inference tracing context.
///
/// This is intentionally a no-op capable handle instead of an `Option` at each
/// transport callsite. Whether tracing is enabled is a session concern; retry,
/// fallback, and stream mapping code should always be able to say what happened
/// without first branching on trace availability.
#[derive(Clone, Debug)]
pub struct InferenceTraceContext {
    state: InferenceTraceContextState,
}

#[derive(Clone, Debug)]
enum InferenceTraceContextState {
    Disabled,
    Enabled(EnabledInferenceTraceContext),
}

#[derive(Clone, Debug)]
struct EnabledInferenceTraceContext {
    writer: Arc<TraceWriter>,
    thread_id: AgentThreadId,
    codex_turn_id: CodexTurnId,
    model: String,
    provider_name: String,
}

/// One concrete upstream request attempt.
///
/// A Codex turn can create multiple attempts when auth recovery retries the
/// HTTP request or WebSocket setup falls back to HTTP. Completion is often
/// observed after the client returns the response stream, so the attempt owns
/// the terminal guard that prevents duplicate lifecycle events.
#[derive(Debug)]
pub struct InferenceTraceAttempt {
    state: InferenceTraceAttemptState,
}

#[derive(Debug)]
enum InferenceTraceAttemptState {
    Disabled,
    Enabled(EnabledInferenceTraceAttempt),
}

#[derive(Debug)]
struct EnabledInferenceTraceAttempt {
    context: EnabledInferenceTraceContext,
    inference_call_id: InferenceCallId,
    terminal_recorded: AtomicBool,
}

/// Non-delta response payload saved for completed or interrupted inference streams.
///
/// We intentionally record completed output items instead of every stream delta
/// here. The raw stream can be added later as a separate payload class; this
/// response summary gives the reducer stable response identity when available
/// plus model-visible output without duplicating high-volume text deltas.
#[derive(Serialize)]
struct TracedResponseStreamOutput<'a> {
    response_id: Option<&'a str>,
    upstream_request_id: Option<&'a str>,
    token_usage: Option<&'a TokenUsage>,
    output_items: Vec<JsonValue>,
}

impl InferenceTraceContext {
    /// Builds a context that accepts trace calls and records nothing.
    pub fn disabled() -> Self {
        Self {
            state: InferenceTraceContextState::Disabled,
        }
    }

    /// Builds an enabled context for all upstream attempts made by one Codex turn.
    pub fn enabled(
        writer: Arc<TraceWriter>,
        thread_id: AgentThreadId,
        codex_turn_id: CodexTurnId,
        model: String,
        provider_name: String,
    ) -> Self {
        Self {
            state: InferenceTraceContextState::Enabled(EnabledInferenceTraceContext {
                writer,
                thread_id,
                codex_turn_id,
                model,
                provider_name,
            }),
        }
    }

    /// Starts a new attempt after the concrete provider request has been built.
    pub fn start_attempt(&self) -> InferenceTraceAttempt {
        let InferenceTraceContextState::Enabled(context) = &self.state else {
            return InferenceTraceAttempt::disabled();
        };

        InferenceTraceAttempt {
            state: InferenceTraceAttemptState::Enabled(EnabledInferenceTraceAttempt {
                context: context.clone(),
                inference_call_id: next_inference_call_id(),
                terminal_recorded: AtomicBool::new(false),
            }),
        }
    }
}

impl InferenceTraceAttempt {
    /// Builds an attempt that records nothing.
    pub fn disabled() -> Self {
        Self {
            state: InferenceTraceAttemptState::Disabled,
        }
    }

    /// Records the exact request object about to be sent to the model provider.
    pub fn record_started(&self, request: &impl Serialize) {
        let InferenceTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        let Some(request_payload) = write_json_payload_best_effort(
            &attempt.context.writer,
            RawPayloadKind::InferenceRequest,
            request,
        ) else {
            return;
        };

        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::InferenceStarted {
                inference_call_id: attempt.inference_call_id.clone(),
                thread_id: attempt.context.thread_id.clone(),
                codex_turn_id: attempt.context.codex_turn_id.clone(),
                model: attempt.context.model.clone(),
                provider_name: attempt.context.provider_name.clone(),
                request_payload,
            },
        );
    }

    /// Records successful provider completion and serializes the observed output items.
    ///
    /// Callers pass protocol-native response items so this crate owns the
    /// trace-specific serialization rules. That keeps codex-core focused on
    /// transport behavior while preserving trace evidence that normal request
    /// serialization intentionally omits.
    pub fn record_completed(
        &self,
        response_id: &str,
        upstream_request_id: Option<&str>,
        token_usage: &Option<TokenUsage>,
        output_items: &[ResponseItem],
    ) {
        let Some(attempt) = self.take_terminal_attempt() else {
            return;
        };
        let Some(response_payload) = write_response_payload_best_effort(
            attempt,
            Some(response_id),
            upstream_request_id,
            token_usage.as_ref(),
            output_items,
        ) else {
            return;
        };

        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::InferenceCompleted {
                inference_call_id: attempt.inference_call_id.clone(),
                response_id: Some(response_id.to_string()),
                upstream_request_id: upstream_request_id.map(str::to_string),
                response_payload,
            },
        );
    }

    /// Records pre-response and mid-stream failures.
    pub fn record_failed(
        &self,
        error: impl Display,
        upstream_request_id: Option<&str>,
        output_items: &[ResponseItem],
    ) {
        let Some(attempt) = self.take_terminal_attempt() else {
            return;
        };
        let partial_response_payload = if output_items.is_empty() {
            None
        } else {
            write_response_payload_best_effort(
                attempt,
                /*response_id*/ None,
                upstream_request_id,
                /*token_usage*/ None,
                output_items,
            )
        };
        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::InferenceFailed {
                inference_call_id: attempt.inference_call_id.clone(),
                upstream_request_id: upstream_request_id.map(str::to_string),
                error: error.to_string(),
                partial_response_payload,
            },
        );
    }

    /// Records a provider stream that Codex intentionally stopped consuming.
    ///
    /// This happens when the turn is interrupted or when mailbox delivery
    /// preempts the current sampling request. Complete output items observed
    /// before that point are retained as partial response evidence.
    pub fn record_cancelled(
        &self,
        reason: impl Display,
        upstream_request_id: Option<&str>,
        output_items: &[ResponseItem],
    ) {
        let Some(attempt) = self.take_terminal_attempt() else {
            return;
        };
        let partial_response_payload = if output_items.is_empty() {
            None
        } else {
            write_response_payload_best_effort(
                attempt,
                /*response_id*/ None,
                upstream_request_id,
                /*token_usage*/ None,
                output_items,
            )
        };
        append_with_context_best_effort(
            &attempt.context,
            RawTraceEventPayload::InferenceCancelled {
                inference_call_id: attempt.inference_call_id.clone(),
                upstream_request_id: upstream_request_id.map(str::to_string),
                reason: reason.to_string(),
                partial_response_payload,
            },
        );
    }

    fn take_terminal_attempt(&self) -> Option<&EnabledInferenceTraceAttempt> {
        let attempt = match &self.state {
            InferenceTraceAttemptState::Disabled => return None,
            InferenceTraceAttemptState::Enabled(attempt) => attempt,
        };
        if attempt.terminal_recorded.swap(true, Ordering::AcqRel) {
            return None;
        }
        Some(attempt)
    }
}

/// Serializes a response item for trace evidence rather than future request construction.
///
/// The protocol serializer intentionally omits some readable reasoning content
/// when shaping items for later model requests. Rollout traces need the item as
/// Codex received it, so this helper restores that content in the raw payload.
pub(crate) fn trace_response_item_json(item: &ResponseItem) -> JsonValue {
    let mut value = serde_json::to_value(item).unwrap_or_else(|err| {
        serde_json::json!({
            "serialization_error": err.to_string(),
        })
    });

    if let ResponseItem::Reasoning {
        content: Some(content),
        ..
    } = item
        && let JsonValue::Object(object) = &mut value
    {
        object.insert(
            "content".to_string(),
            serde_json::to_value(content).unwrap_or_else(|err| {
                serde_json::json!({
                    "serialization_error": err.to_string(),
                })
            }),
        );
    }

    value
}

fn next_inference_call_id() -> InferenceCallId {
    let ordinal = NEXT_INFERENCE_ATTEMPT.fetch_add(1, Ordering::Relaxed);
    format!("inference:{ordinal}")
}

fn write_json_payload_best_effort(
    writer: &TraceWriter,
    kind: RawPayloadKind,
    payload: &impl Serialize,
) -> Option<crate::RawPayloadRef> {
    writer.write_json_payload(kind, payload).ok()
}

fn write_response_payload_best_effort(
    attempt: &EnabledInferenceTraceAttempt,
    response_id: Option<&str>,
    upstream_request_id: Option<&str>,
    token_usage: Option<&TokenUsage>,
    output_items: &[ResponseItem],
) -> Option<crate::RawPayloadRef> {
    let response_payload = TracedResponseStreamOutput {
        response_id,
        upstream_request_id,
        token_usage,
        output_items: output_items.iter().map(trace_response_item_json).collect(),
    };
    write_json_payload_best_effort(
        &attempt.context.writer,
        RawPayloadKind::InferenceResponse,
        &response_payload,
    )
}

fn append_with_context_best_effort(
    context: &EnabledInferenceTraceContext,
    payload: RawTraceEventPayload,
) {
    let event_context = RawTraceEventContext {
        thread_id: Some(context.thread_id.clone()),
        codex_turn_id: Some(context.codex_turn_id.clone()),
    };
    let _ = context.writer.append_with_context(event_context, payload);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_protocol::models::ReasoningItemContent;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::model::ExecutionStatus;
    use crate::replay_bundle;

    #[test]
    fn enabled_context_records_replayable_inference_attempt() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let writer = Arc::new(TraceWriter::create(
            temp.path(),
            "trace-1".to_string(),
            "rollout-1".to_string(),
            "thread-root".to_string(),
        )?);
        writer.append(RawTraceEventPayload::ThreadStarted {
            thread_id: "thread-root".to_string(),
            agent_path: "/root".to_string(),
            metadata_payload: None,
        })?;
        writer.append(RawTraceEventPayload::CodexTurnStarted {
            codex_turn_id: "turn-1".to_string(),
            thread_id: "thread-root".to_string(),
        })?;
        let context = InferenceTraceContext::enabled(
            writer,
            "thread-root".to_string(),
            "turn-1".to_string(),
            "gpt-test".to_string(),
            "test-provider".to_string(),
        );

        let attempt = context.start_attempt();
        attempt.record_started(&json!({
            "model": "gpt-test",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}]
            }],
        }));
        attempt.record_completed("resp-1", Some("req-1"), &None, &[]);

        let rollout = replay_bundle(temp.path())?;
        let inference = rollout
            .inference_calls
            .values()
            .next()
            .expect("recorded inference call");

        assert_eq!(rollout.inference_calls.len(), 1);
        assert_eq!(inference.thread_id, "thread-root");
        assert_eq!(inference.codex_turn_id, "turn-1");
        assert_eq!(inference.execution.status, ExecutionStatus::Completed);
        assert_eq!(inference.upstream_request_id, Some("req-1".to_string()));
        assert_eq!(rollout.raw_payloads.len(), 2);

        Ok(())
    }

    #[test]
    fn traced_response_item_preserves_reasoning_content_omitted_by_normal_serializer() {
        let item = ResponseItem::Reasoning {
            id: "rs-1".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "summary".to_string(),
            }],
            content: Some(vec![ReasoningItemContent::Text {
                text: "raw reasoning".to_string(),
            }]),
            encrypted_content: Some("encoded".to_string()),
        };

        let normal = serde_json::to_value(&item).expect("response item serializes");
        let traced = trace_response_item_json(&item);

        assert_eq!(normal.get("content"), None);
        assert_eq!(
            traced,
            json!({
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "summary"}],
                "content": [{"type": "text", "text": "raw reasoning"}],
                "encrypted_content": "encoded",
            }),
        );
    }
}
