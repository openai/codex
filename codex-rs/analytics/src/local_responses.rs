use crate::LOCAL_ANALYTICS_SCHEMA_VERSION;
use crate::LocalAnalyticsRecord;
use crate::LocalAnalyticsRecordType;
use crate::facts::AnalyticsFact;
use crate::now_unix_millis;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;

static NEXT_LOCAL_RESPONSES_CALL_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) enum AnalyticsQueueInput {
    AnalyticsFact(AnalyticsFact),
    LocalResponsesApiCallStarted(LocalResponsesApiCallStartedFact),
    LocalResponsesApiCallTerminal(LocalResponsesApiCallTerminalFact),
}

/// Transport used for one locally captured Responses API attempt.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalResponsesApiTransport {
    Http,
    Websocket,
}

/// No-op-capable turn-local factory for locally captured Responses API attempts.
#[derive(Clone)]
pub struct LocalResponsesApiCallCapture {
    sender: Option<mpsc::Sender<AnalyticsQueueInput>>,
    session_id: String,
    thread_id: String,
    turn_id: String,
    context_window_id: String,
}

/// One locally captured Responses API attempt.
pub struct LocalResponsesApiCallAttempt {
    sender: Option<mpsc::Sender<AnalyticsQueueInput>>,
    responses_call_id: String,
    terminal_recorded: AtomicBool,
}

#[derive(Debug)]
pub(crate) struct LocalResponsesApiCallStartedFact {
    responses_call_id: String,
    session_id: String,
    thread_id: String,
    turn_id: String,
    context_window_id: String,
    transport: LocalResponsesApiTransport,
    request_started_at_epoch_millis: u64,
    request_json: JsonValue,
}

#[derive(Debug)]
pub(crate) enum LocalResponsesApiCallTerminalFact {
    Completed {
        responses_call_id: String,
        completed_at_epoch_millis: u64,
        response_id: String,
        upstream_request_id: Option<String>,
        token_usage_json: Option<JsonValue>,
        output_items: Vec<JsonValue>,
    },
    Failed {
        responses_call_id: String,
        completed_at_epoch_millis: u64,
        upstream_request_id: Option<String>,
        error_json: JsonValue,
        output_items: Vec<JsonValue>,
    },
    Cancelled {
        responses_call_id: String,
        completed_at_epoch_millis: u64,
        upstream_request_id: Option<String>,
        error_json: JsonValue,
        output_items: Vec<JsonValue>,
    },
}

#[derive(Default)]
pub(crate) struct LocalResponsesApiCallReducer {
    started_calls: HashMap<String, LocalResponsesApiCallStartedFact>,
}

#[derive(Serialize)]
struct LocalResponsesApiCallPayload {
    responses_call_id: String,
    context_window_id: String,
    transport: LocalResponsesApiTransport,
    status: LocalResponsesApiCallStatus,
    request_started_at_epoch_millis: u64,
    completed_at_epoch_millis: u64,
    response_id: Option<String>,
    upstream_request_id: Option<String>,
    request_json: JsonValue,
    response_json: Option<JsonValue>,
    token_usage_json: Option<JsonValue>,
    error_json: Option<JsonValue>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocalResponsesApiCallStatus {
    Completed,
    Failed,
    Cancelled,
}

impl LocalResponsesApiCallCapture {
    /// Builds a capture handle that accepts calls and records nothing.
    pub fn disabled() -> Self {
        Self {
            sender: None,
            session_id: String::new(),
            thread_id: String::new(),
            turn_id: String::new(),
            context_window_id: String::new(),
        }
    }

    pub(crate) fn enabled(
        sender: mpsc::Sender<AnalyticsQueueInput>,
        session_id: String,
        thread_id: String,
        turn_id: String,
        context_window_id: String,
    ) -> Self {
        Self {
            sender: Some(sender),
            session_id,
            thread_id,
            turn_id,
            context_window_id,
        }
    }

    /// Starts one locally captured Responses API attempt.
    pub fn start_attempt(
        &self,
        transport: LocalResponsesApiTransport,
        request: &impl Serialize,
    ) -> LocalResponsesApiCallAttempt {
        let Some(sender) = self.sender.as_ref() else {
            return LocalResponsesApiCallAttempt::disabled();
        };
        let Some(request_json) = serialize_json_best_effort(request, "request") else {
            return LocalResponsesApiCallAttempt::disabled();
        };
        let responses_call_id = next_local_responses_call_id();
        try_send_best_effort(
            sender,
            AnalyticsQueueInput::LocalResponsesApiCallStarted(LocalResponsesApiCallStartedFact {
                responses_call_id: responses_call_id.clone(),
                session_id: self.session_id.clone(),
                thread_id: self.thread_id.clone(),
                turn_id: self.turn_id.clone(),
                context_window_id: self.context_window_id.clone(),
                transport,
                request_started_at_epoch_millis: now_unix_millis(),
                request_json,
            }),
        );
        LocalResponsesApiCallAttempt {
            sender: Some(sender.clone()),
            responses_call_id,
            terminal_recorded: AtomicBool::new(false),
        }
    }
}

impl LocalResponsesApiCallAttempt {
    /// Builds an attempt that accepts terminal calls and records nothing.
    pub fn disabled() -> Self {
        Self {
            sender: None,
            responses_call_id: String::new(),
            terminal_recorded: AtomicBool::new(false),
        }
    }

    /// Records successful provider completion and observed non-delta output items.
    pub fn record_completed(
        &self,
        response_id: &str,
        upstream_request_id: Option<&str>,
        token_usage: &Option<TokenUsage>,
        output_items: &[ResponseItem],
    ) {
        self.record_terminal(LocalResponsesApiCallTerminalFact::Completed {
            responses_call_id: self.responses_call_id.clone(),
            completed_at_epoch_millis: now_unix_millis(),
            response_id: response_id.to_string(),
            upstream_request_id: upstream_request_id.map(str::to_string),
            token_usage_json: token_usage
                .as_ref()
                .and_then(|usage| serialize_json_best_effort(usage, "token usage")),
            output_items: serialize_response_items(output_items),
        });
    }

    /// Records a failed provider attempt and any observed non-delta output items.
    pub fn record_failed(
        &self,
        error: impl Display,
        upstream_request_id: Option<&str>,
        output_items: &[ResponseItem],
    ) {
        self.record_terminal(LocalResponsesApiCallTerminalFact::Failed {
            responses_call_id: self.responses_call_id.clone(),
            completed_at_epoch_millis: now_unix_millis(),
            upstream_request_id: upstream_request_id.map(str::to_string),
            error_json: json!({ "message": error.to_string() }),
            output_items: serialize_response_items(output_items),
        });
    }

    /// Records a provider stream that Codex intentionally stopped consuming.
    pub fn record_cancelled(
        &self,
        reason: impl Display,
        upstream_request_id: Option<&str>,
        output_items: &[ResponseItem],
    ) {
        self.record_terminal(LocalResponsesApiCallTerminalFact::Cancelled {
            responses_call_id: self.responses_call_id.clone(),
            completed_at_epoch_millis: now_unix_millis(),
            upstream_request_id: upstream_request_id.map(str::to_string),
            error_json: json!({ "reason": reason.to_string() }),
            output_items: serialize_response_items(output_items),
        });
    }

    fn record_terminal(&self, terminal: LocalResponsesApiCallTerminalFact) {
        let Some(sender) = self.sender.as_ref() else {
            return;
        };
        if self.terminal_recorded.swap(true, Ordering::AcqRel) {
            return;
        }
        try_send_best_effort(
            sender,
            AnalyticsQueueInput::LocalResponsesApiCallTerminal(terminal),
        );
    }
}

impl LocalResponsesApiCallReducer {
    pub(crate) fn ingest_started(&mut self, started: LocalResponsesApiCallStartedFact) {
        self.started_calls
            .insert(started.responses_call_id.clone(), started);
    }

    pub(crate) fn ingest_terminal(
        &mut self,
        terminal: LocalResponsesApiCallTerminalFact,
    ) -> Option<LocalAnalyticsRecord> {
        let responses_call_id = terminal.responses_call_id();
        let Some(started) = self.started_calls.remove(responses_call_id) else {
            tracing::warn!(
                responses_call_id,
                "dropping local Responses terminal without matching start"
            );
            return None;
        };
        Some(terminal.into_record(started))
    }
}

impl LocalResponsesApiCallTerminalFact {
    fn responses_call_id(&self) -> &str {
        match self {
            Self::Completed {
                responses_call_id, ..
            }
            | Self::Failed {
                responses_call_id, ..
            }
            | Self::Cancelled {
                responses_call_id, ..
            } => responses_call_id,
        }
    }

    fn into_record(self, started: LocalResponsesApiCallStartedFact) -> LocalAnalyticsRecord {
        let (
            status,
            completed_at_epoch_millis,
            response_id,
            upstream_request_id,
            response_json,
            token_usage_json,
            error_json,
        ) = match self {
            Self::Completed {
                completed_at_epoch_millis,
                response_id,
                upstream_request_id,
                token_usage_json,
                output_items,
                ..
            } => (
                LocalResponsesApiCallStatus::Completed,
                completed_at_epoch_millis,
                Some(response_id.clone()),
                upstream_request_id.clone(),
                Some(response_summary(
                    Some(response_id),
                    upstream_request_id,
                    output_items,
                )),
                token_usage_json,
                None,
            ),
            Self::Failed {
                completed_at_epoch_millis,
                upstream_request_id,
                error_json,
                output_items,
                ..
            } => (
                LocalResponsesApiCallStatus::Failed,
                completed_at_epoch_millis,
                None,
                upstream_request_id.clone(),
                partial_response_summary(upstream_request_id, output_items),
                None,
                Some(error_json),
            ),
            Self::Cancelled {
                completed_at_epoch_millis,
                upstream_request_id,
                error_json,
                output_items,
                ..
            } => (
                LocalResponsesApiCallStatus::Cancelled,
                completed_at_epoch_millis,
                None,
                upstream_request_id.clone(),
                partial_response_summary(upstream_request_id, output_items),
                None,
                Some(error_json),
            ),
        };
        let payload = LocalResponsesApiCallPayload {
            responses_call_id: started.responses_call_id,
            context_window_id: started.context_window_id,
            transport: started.transport,
            status,
            request_started_at_epoch_millis: started.request_started_at_epoch_millis,
            completed_at_epoch_millis,
            response_id,
            upstream_request_id,
            request_json: started.request_json,
            response_json,
            token_usage_json,
            error_json,
        };
        LocalAnalyticsRecord {
            schema_version: LOCAL_ANALYTICS_SCHEMA_VERSION,
            recorded_at_epoch_millis: completed_at_epoch_millis,
            record_type: LocalAnalyticsRecordType::ResponsesApiCall,
            session_id: Some(started.session_id),
            thread_id: Some(started.thread_id),
            turn_id: Some(started.turn_id),
            payload: serde_json::to_value(payload).unwrap_or_else(|err| {
                json!({
                    "serialization_error": err.to_string(),
                })
            }),
        }
    }
}

fn response_summary(
    response_id: Option<String>,
    upstream_request_id: Option<String>,
    output_items: Vec<JsonValue>,
) -> JsonValue {
    json!({
        "response_id": response_id,
        "upstream_request_id": upstream_request_id,
        "output_items": output_items,
    })
}

fn partial_response_summary(
    upstream_request_id: Option<String>,
    output_items: Vec<JsonValue>,
) -> Option<JsonValue> {
    (!output_items.is_empty()).then(|| response_summary(None, upstream_request_id, output_items))
}

fn serialize_response_items(output_items: &[ResponseItem]) -> Vec<JsonValue> {
    output_items
        .iter()
        .filter_map(|item| serialize_json_best_effort(item, "response item"))
        .collect()
}

fn serialize_json_best_effort(value: &impl Serialize, kind: &str) -> Option<JsonValue> {
    match serde_json::to_value(value) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(error = %err, "failed to serialize local Responses {kind}");
            None
        }
    }
}

fn try_send_best_effort(sender: &mpsc::Sender<AnalyticsQueueInput>, input: AnalyticsQueueInput) {
    if sender.try_send(input).is_err() {
        tracing::warn!("dropping local Responses analytics input: queue is full");
    }
}

fn next_local_responses_call_id() -> String {
    let sequence = NEXT_LOCAL_RESPONSES_CALL_ID.fetch_add(1, Ordering::Relaxed);
    format!("local-responses-{}-{sequence}", now_unix_millis())
}

#[cfg(test)]
#[path = "local_responses_tests.rs"]
mod tests;
