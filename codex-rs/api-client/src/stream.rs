use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::TokenUsage;
use futures::Stream;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::error::Result;

#[derive(Debug, Serialize, Clone)]
pub struct Reasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ReasoningSummaryConfig>,
}

#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TextFormatType {
    #[default]
    JsonSchema,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct TextFormat {
    pub r#type: TextFormatType,
    pub strict: bool,
    pub schema: Value,
    pub name: String,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct TextControls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<TextFormat>,
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    OutputItemAdded(ResponseItem),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    RateLimits(RateLimitSnapshot),
}

#[derive(Debug)]
pub struct EventStream<T> {
    pub(crate) rx_event: mpsc::Receiver<T>,
}

impl<T> EventStream<T> {
    pub fn from_receiver(rx_event: mpsc::Receiver<T>) -> Self {
        Self { rx_event }
    }
}

impl<T> Stream for EventStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

pub type ResponseStream = EventStream<Result<ResponseEvent>>;

#[derive(Debug, Clone)]
pub struct WireTokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone)]
pub struct WireRateLimitWindow {
    pub used_percent: Option<f64>,
    pub window_minutes: Option<i64>,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct WireRateLimitSnapshot {
    pub primary: Option<WireRateLimitWindow>,
    pub secondary: Option<WireRateLimitWindow>,
}

#[derive(Debug)]
pub enum WireEvent {
    Created,
    OutputItemDone(serde_json::Value),
    OutputItemAdded(serde_json::Value),
    Completed {
        response_id: String,
        token_usage: Option<WireTokenUsage>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    RateLimits(WireRateLimitSnapshot),
}

pub type WireResponseStream = EventStream<Result<WireEvent>>;
