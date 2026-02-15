//! A2A protocol error type with JSON-RPC error codes.
//!
//! Mirrors `a2a-js/src/server/error.ts`.

use axum::response::IntoResponse;
use serde::Serialize;

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A2A error with JSON-RPC error codes.
#[derive(Debug)]
pub struct A2AError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub task_id: Option<String>,
}

impl A2AError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
            task_id: None,
        }
    }

    /// Convert to JSON-RPC error object.
    pub fn to_jsonrpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.code,
            message: self.message.clone(),
            data: self.data.clone(),
        }
    }

    // ====== Factory methods (same as a2a-js) ======

    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(-32700, message)
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(-32600, message)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {method}"))
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(-32603, message)
    }

    pub fn task_not_found(task_id: &str) -> Self {
        let mut e = Self::new(-32001, format!("Task not found: {task_id}"));
        e.task_id = Some(task_id.to_string());
        e
    }

    pub fn task_not_cancelable(task_id: &str) -> Self {
        let mut e = Self::new(-32002, format!("Task not cancelable: {task_id}"));
        e.task_id = Some(task_id.to_string());
        e
    }

    pub fn push_notification_not_supported() -> Self {
        Self::new(-32003, "Push Notification is not supported")
    }

    pub fn unsupported_operation(operation: &str) -> Self {
        Self::new(-32004, format!("Unsupported operation: {operation}"))
    }
}

impl std::fmt::Display for A2AError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "A2AError({}): {}", self.code, self.message)
    }
}

impl std::error::Error for A2AError {}

impl IntoResponse for A2AError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code {
            -32001 => axum::http::StatusCode::NOT_FOUND,
            -32600 | -32602 => axum::http::StatusCode::BAD_REQUEST,
            -32601 => axum::http::StatusCode::METHOD_NOT_ALLOWED,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = serde_json::json!({ "error": self.to_jsonrpc_error() });
        (status, axum::Json(body)).into_response()
    }
}
