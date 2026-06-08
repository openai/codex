//! Types used when representing Model Context Protocol (MCP) values inside the
//! Codex protocol.
//!
//! We intentionally keep these types TS/JSON-schema friendly (via `ts-rs` and
//! `schemars`) so they can be embedded in Codex's own protocol structures.
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// ID of a request, which can be either a string or an integer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    #[ts(type = "number")]
    Integer(i64),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::String(s) => f.write_str(s),
            RequestId::Integer(i) => i.fmt(f),
        }
    }
}

/// Presentation metadata advertised by an initialized MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub name: String,
    pub title: Option<String>,
    pub version: String,
    pub description: Option<String>,
    pub icons: Option<Vec<serde_json::Value>>,
    pub website_url: Option<String>,
}

/// Definition for a tool the client can call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<serde_json::Value>,
}

/// A known resource that the server is capable of reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mime_type: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[ts(type = "number")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<serde_json::Value>,
}

/// Contents returned when reading a resource from an MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum ResourceContent {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Text {
        /// The URI of this resource.
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        mime_type: Option<String>,
        text: String,
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Blob {
        /// The URI of this resource.
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        mime_type: Option<String>,
        blob: String,
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        meta: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadResourceResult {
    pub contents: Vec<ResourceContent>,
}

/// A template description for resources available on the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub annotations: Option<serde_json::Value>,
    pub uri_template: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mime_type: Option<String>,
}

/// The server's response to a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub structured_content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub is_error: Option<bool>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<serde_json::Value>,
}
