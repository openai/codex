use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(rename = "isOther", default)]
    #[schemars(rename = "isOther")]
    #[ts(rename = "isOther")]
    pub is_other: bool,
    #[serde(rename = "isSecret", default)]
    #[schemars(rename = "isSecret")]
    #[ts(rename = "isSecret")]
    pub is_secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<RequestUserInputQuestionOption>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputArgs {
    pub questions: Vec<RequestUserInputQuestion>,
    #[serde(rename = "isBlocking")]
    #[schemars(rename = "isBlocking")]
    #[ts(rename = "isBlocking")]
    pub is_blocking: bool,
    /// @deprecated Use `isBlocking` to decide whether the request should block.
    #[serde(rename = "autoResolutionMs", skip_serializing_if = "Option::is_none")]
    #[schemars(rename = "autoResolutionMs")]
    pub auto_resolution_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputResponse {
    pub answers: HashMap<String, RequestUserInputAnswer>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, JsonSchema, TS)]
pub struct RequestUserInputEvent {
    /// Responses API call id for the associated tool call, if available.
    pub call_id: String,
    /// Turn ID that this request belongs to.
    /// Uses `#[serde(default)]` for backwards compatibility.
    #[serde(default)]
    pub turn_id: String,
    pub questions: Vec<RequestUserInputQuestion>,
    #[serde(rename = "isBlocking")]
    #[schemars(rename = "isBlocking")]
    #[ts(rename = "isBlocking")]
    pub is_blocking: bool,
    /// @deprecated Use `isBlocking` to decide whether the request should block.
    #[serde(rename = "autoResolutionMs", skip_serializing_if = "Option::is_none")]
    #[schemars(rename = "autoResolutionMs")]
    pub auto_resolution_ms: Option<u64>,
}

impl<'de> Deserialize<'de> for RequestUserInputEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireRequestUserInputEvent {
            call_id: String,
            #[serde(default)]
            turn_id: String,
            questions: Vec<RequestUserInputQuestion>,
            #[serde(rename = "isBlocking")]
            is_blocking: Option<bool>,
            #[serde(rename = "autoResolutionMs")]
            auto_resolution_ms: Option<u64>,
        }

        let wire = WireRequestUserInputEvent::deserialize(deserializer)?;
        Ok(Self {
            call_id: wire.call_id,
            turn_id: wire.turn_id,
            questions: wire.questions,
            is_blocking: wire.is_blocking.unwrap_or(true),
            auto_resolution_ms: wire.auto_resolution_ms,
        })
    }
}

#[cfg(test)]
#[path = "request_user_input_tests.rs"]
mod tests;
