use crate::error::Result;
use crate::models::ResponseItem;
use futures::Stream;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// API request payload for a single model turn.
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,
    /// Optional previous response ID (when storage is enabled).
    pub prev_id: Option<String>,
    /// Optional instructions from the user to amend to the built-in agent
    /// instructions.
    pub instructions: Option<String>,
    /// Whether to store response on server side (disable_response_storage = !store).
    pub store: bool,

    /// Additional tools sourced from external MCP servers. Note each key is
    /// the "fully qualified" tool name (i.e., prefixed with the server name),
    /// which should be reported to the model in place of Tool::name.
    pub extra_tools: HashMap<String, mcp_types::Tool>,
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self) -> Cow<str> {
        match &self.instructions {
            Some(instructions) => {
                let instructions = format!("{BASE_INSTRUCTIONS}\n{instructions}");
                Cow::Owned(instructions)
            }
            None => Cow::Borrowed(BASE_INSTRUCTIONS),
        }
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    OutputItemDone(ResponseItem),
    Completed { response_id: String },
}

#[derive(Debug, Serialize)]
pub(crate) struct Reasoning {
    pub(crate) effort: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<Summary>,
}

/// A summary of the reasoning performed by the model. This can be useful for
/// debugging and understanding the model's reasoning process.
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Summary {
    Auto,
    #[allow(dead_code)] // Will go away once this is configurable.
    Concise,
    #[allow(dead_code)] // Will go away once this is configurable.
    Detailed,
}

#[derive(Debug, Serialize)]
pub(crate) struct Payload<'a> {
    pub(crate) model: &'a str,
    pub(crate) instructions: &'a str,
    // TODO(mbolin): ResponseItem::Other should not be serialized. Currently,
    // we code defensively to avoid this case, but perhaps we should use a
    // separate enum for serialization.
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) previous_response_id: Option<String>,
    /// true when using the Responses API.
    pub(crate) store: bool,
    pub(crate) stream: bool,
}

pub(crate) struct ResponseStream {
    pub(crate) rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}
