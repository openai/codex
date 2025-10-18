//! We do not do true JSON-RPC 2.0, as we neither send nor expect the
//! "jsonrpc": "2.0" field.

use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

pub const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Hash, Eq, TS)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    #[ts(type = "number")]
    Integer(i64),
}

pub type Result = serde_json::Value;

/// Refers to any valid JSON-RPC object that can be decoded off the wire, or encoded to be sent.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum JSONRPCMessage {
    Request(JSONRPCRequest),
    Notification(JSONRPCNotification),
    Response(JSONRPCResponse),
    Error(JSONRPCError),
}

/// A request that expects a response.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct JSONRPCRequest {
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A notification which does not expect a response.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct JSONRPCNotification {
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A successful (non-error) response to a request.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct JSONRPCResponse {
    pub id: RequestId,
    pub result: Result,
}

/// A response to a request that indicates an error occurred.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct JSONRPCError {
    pub error: JSONRPCErrorError,
    pub id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
pub struct JSONRPCErrorError {
    pub code: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub message: String,
}
