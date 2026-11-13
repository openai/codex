//! Hot-path helpers for recording upstream inference attempts.
//!
//! A disabled context records nothing, which keeps one-shot HTTP calls,
//! WebSocket reuse, and retry/fallback attempts on the same lifecycle path.
//! Transport callers can check whether terminal records need output items
//! before retaining them.

use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use http::HeaderMap;
use http::HeaderValue;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error;
use serde::ser::SerializeSeq;
use uuid::Uuid;

use crate::model::AgentThreadId;
use crate::model::CodexTurnId;
use crate::model::InferenceCallId;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;
use crate::writer::TraceWriter;

const INFERENCE_CALL_ID_HEADER: &str = "x-codex-inference-call-id";

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
    awaiting_terminal: AtomicBool,
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
    #[serde(serialize_with = "serialize_trace_items")]
    output_items: &'a [ResponseItem],
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
                awaiting_terminal: AtomicBool::new(false),
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

    /// Whether terminal trace records need the completed output items.
    pub fn needs_output_items(&self) -> bool {
        matches!(&self.state, InferenceTraceAttemptState::Enabled(_))
    }

    fn inference_call_id(&self) -> Option<&str> {
        match &self.state {
            InferenceTraceAttemptState::Disabled => None,
            InferenceTraceAttemptState::Enabled(attempt) => {
                Some(attempt.inference_call_id.as_str())
            }
        }
    }

    /// Adds rollout-trace propagation headers for this attempt when tracing is enabled.
    pub fn add_request_headers(&self, headers: &mut HeaderMap) {
        let Some(inference_call_id) = self.inference_call_id() else {
            return;
        };
        let Ok(inference_call_id) = HeaderValue::from_str(inference_call_id) else {
            // These IDs are generated internally as UUID strings, so rejection
            // should be impossible in practice. Tracing remains best-effort,
            // though, and must never make provider requests fail.
            return;
        };

        headers.insert(INFERENCE_CALL_ID_HEADER, inference_call_id);
    }

    /// Captures the transport's encoded JSON and the immutable Core prompt that produced it.
    /// A continuation from an unrecorded warmup also supplies its full formatted context.
    pub fn record_started<P: Serialize + Send + Sync + 'static>(
        &self,
        outgoing: impl AsRef<[u8]> + Send + 'static,
        prepared_prompt: Arc<P>,
        warmup_context: impl FnOnce() -> serde_json::Result<Option<Vec<u8>>> + Send + 'static,
    ) {
        let InferenceTraceAttemptState::Enabled(attempt) = &self.state else {
            return;
        };
        let ctx = &attempt.context;
        let thread_id = ctx.thread_id.clone();
        let codex_turn_id = ctx.codex_turn_id.clone();
        let inference_call_id = attempt.inference_call_id.clone();
        let model = ctx.model.clone();
        let provider_name = ctx.provider_name.clone();
        let start = ctx.writer.defer_event(
            RawTraceEventContext::turn(&thread_id, &codex_turn_id),
            move |bundle| {
                let request_payload =
                    bundle.write_body(RawPayloadKind::InferenceRequest, outgoing.as_ref())?;
                let prepared_prompt_payload = bundle.write_body(
                    RawPayloadKind::PreparedPrompt,
                    &serde_json::to_vec(prepared_prompt.as_ref())?,
                )?;
                let warmup_context_payload = warmup_context()?
                    .map(|bytes| bundle.write_body(RawPayloadKind::WarmupContext, &bytes))
                    .transpose()?;
                Ok(RawTraceEventPayload::InferenceStarted {
                    inference_call_id,
                    thread_id,
                    codex_turn_id,
                    model,
                    provider_name,
                    request_payload,
                    prepared_prompt_payload,
                    warmup_context_payload,
                })
            },
        );
        // A transport encoding error produces no outgoing request or start row.
        attempt
            .awaiting_terminal
            .store(start.is_ok(), Ordering::Release);
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
        let payload = TracedResponseStreamOutput {
            response_id: Some(response_id),
            upstream_request_id,
            token_usage: token_usage.as_ref(),
            output_items,
        };
        let inference_call_id = attempt.inference_call_id.clone();
        let response_id = Some(response_id.to_owned());
        let upstream_request_id = upstream_request_id.map(str::to_owned);
        let _ = attempt.context.writer.event_with_payload(
            RawTraceEventContext::turn(&attempt.context.thread_id, &attempt.context.codex_turn_id),
            RawPayloadKind::InferenceResponse,
            &payload,
            move |response_payload| RawTraceEventPayload::InferenceCompleted {
                inference_call_id,
                response_id,
                upstream_request_id,
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
        let inference_call_id = attempt.inference_call_id.clone();
        let error = error.to_string();
        let upstream = upstream_request_id.map(str::to_owned);
        record_partial_response(
            attempt,
            upstream_request_id,
            output_items,
            move |partial_response_payload| RawTraceEventPayload::InferenceFailed {
                inference_call_id,
                upstream_request_id: upstream,
                error,
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
        let inference_call_id = attempt.inference_call_id.clone();
        let reason = reason.to_string();
        let upstream = upstream_request_id.map(str::to_owned);
        record_partial_response(
            attempt,
            upstream_request_id,
            output_items,
            move |partial_response_payload| RawTraceEventPayload::InferenceCancelled {
                inference_call_id,
                upstream_request_id: upstream,
                reason,
                partial_response_payload,
            },
        );
    }

    fn take_terminal_attempt(&self) -> Option<&EnabledInferenceTraceAttempt> {
        let attempt = match &self.state {
            InferenceTraceAttemptState::Disabled => return None,
            InferenceTraceAttemptState::Enabled(attempt) => attempt,
        };
        attempt
            .awaiting_terminal
            .swap(false, Ordering::AcqRel)
            .then_some(attempt)
    }
}

/// Serializes Core history or received response items for replay, preserving
/// readable reasoning that the outbound request serializer can omit.
pub fn serialize_trace_items<S: Serializer>(
    items: &[ResponseItem],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(items.len()))?;
    for item in items {
        if let ResponseItem::Reasoning {
            content: Some(content),
            ..
        } = item
        {
            let mut value = serde_json::to_value(item).map_err(S::Error::custom)?;
            value["content"] = serde_json::to_value(content).map_err(S::Error::custom)?;
            sequence.serialize_element(&value)?;
        } else {
            sequence.serialize_element(item)?;
        }
    }
    sequence.end()
}

fn next_inference_call_id() -> InferenceCallId {
    Uuid::new_v4().to_string()
}

fn record_partial_response(
    attempt: &EnabledInferenceTraceAttempt,
    upstream_request_id: Option<&str>,
    output_items: &[ResponseItem],
    event: impl FnOnce(Option<crate::RawPayloadRef>) -> RawTraceEventPayload + Send + 'static,
) {
    let ctx = &attempt.context;
    let envelope = RawTraceEventContext::turn(&ctx.thread_id, &ctx.codex_turn_id);
    if output_items.is_empty() {
        let _ = ctx.writer.enqueue_event(envelope, event(None));
    } else {
        let payload = TracedResponseStreamOutput {
            response_id: None,
            upstream_request_id,
            token_usage: None,
            output_items,
        };
        let _ = ctx.writer.event_with_payload(
            envelope,
            RawPayloadKind::InferenceResponse,
            &payload,
            move |reference| event(Some(reference)),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_protocol::ResponseItemId;
    use codex_protocol::models::ReasoningItemContent;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::model::ExecutionStatus;
    use crate::replay_bundle;

    #[test]
    fn disabled_attempt_adds_no_request_headers() {
        let mut headers = HeaderMap::new();

        InferenceTraceAttempt::disabled().add_request_headers(&mut headers);

        assert!(headers.is_empty());
    }

    #[test]
    fn enabled_attempt_adds_inference_request_header() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let writer = Arc::new(TraceWriter::create(
            temp.path(),
            "trace-1".to_string(),
            "rollout-1".to_string(),
            "thread-root".to_string(),
        )?);
        let context = InferenceTraceContext::enabled(
            writer,
            "thread-root".to_string(),
            "turn-1".to_string(),
            "gpt-test".to_string(),
            "test-provider".to_string(),
        );
        let attempt = context.start_attempt();
        let mut headers = HeaderMap::new();

        attempt.add_request_headers(&mut headers);

        let header = headers
            .get(INFERENCE_CALL_ID_HEADER)
            .expect("inference header present");
        assert_eq!(Some(header.to_str()?), attempt.inference_call_id());
        assert!(Uuid::parse_str(header.to_str()?).is_ok());
        Ok(())
    }

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
            Arc::clone(&writer),
            "thread-root".to_string(),
            "turn-1".to_string(),
            "gpt-test".to_string(),
            "test-provider".to_string(),
        );

        let attempt = context.start_attempt();
        let request = json!({
            "model": "gpt-test",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}]
            }],
        });
        attempt.record_started(serde_json::to_vec(&request)?, Arc::new(request), || {
            Ok(None)
        });
        attempt.record_completed("resp-1", Some("req-1"), &None, &[]);

        writer.flush()?;
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
        assert_eq!(rollout.raw_payloads.len(), 3);

        Ok(())
    }

    #[test]
    fn traced_response_item_preserves_reasoning_content_omitted_by_normal_serializer() {
        let item = ResponseItem::Reasoning {
            id: Some(ResponseItemId::with_suffix("rs", "1")),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "summary".to_string(),
            }],
            content: Some(vec![ReasoningItemContent::Text {
                text: "raw reasoning".to_string(),
            }]),
            encrypted_content: Some("encoded".to_string()),
            internal_chat_message_metadata_passthrough: None,
        };

        let normal = serde_json::to_value(&item).expect("response item serializes");
        let traced =
            serialize_trace_items(std::slice::from_ref(&item), serde_json::value::Serializer)
                .expect("trace item serializes");

        assert_eq!(normal.get("content"), None);
        assert_eq!(
            traced,
            json!([{
                "type": "reasoning",
                "id": "rs_1",
                "summary": [{"type": "summary_text", "text": "summary"}],
                "content": [{"type": "text", "text": "raw reasoning"}],
                "encrypted_content": "encoded",
            }]),
        );
    }
}

#[cfg(test)]
#[path = "inference_recording_tests.rs"]
mod recording_tests;
