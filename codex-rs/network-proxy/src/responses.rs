use crate::reasons::REASON_DENIED;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
use rama_http::Body;
use rama_http::Response;
use rama_http::StatusCode;
use serde::Serialize;
use tracing::error;

pub fn text_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}

pub fn json_response<T: Serialize>(value: &T) -> Response {
    let body = match serde_json::to_string(value) {
        Ok(body) => body,
        Err(err) => {
            error!("failed to serialize JSON response: {err}");
            "{}".to_string()
        }
    };
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|err| {
            error!("failed to build JSON response: {err}");
            Response::new(Body::from("{}"))
        })
}

pub fn blocked_header_value(reason: &str) -> &'static str {
    match reason {
        REASON_NOT_ALLOWED | REASON_NOT_ALLOWED_LOCAL => "blocked-by-allowlist",
        REASON_DENIED => "blocked-by-denylist",
        REASON_METHOD_NOT_ALLOWED => "blocked-by-method-policy",
        _ => "blocked-by-policy",
    }
}

pub fn blocked_message(reason: &str) -> &'static str {
    match reason {
        REASON_NOT_ALLOWED => "Codex blocked this request: domain not in allowlist.",
        REASON_NOT_ALLOWED_LOCAL => {
            "Codex blocked this request: local/private addresses not allowed."
        }
        REASON_DENIED => "Codex blocked this request: domain denied by policy.",
        REASON_METHOD_NOT_ALLOWED => {
            "Codex blocked this request: method not allowed in limited mode."
        }
        _ => "Codex blocked this request by network policy.",
    }
}

pub fn blocked_text_response(reason: &str) -> Response {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("content-type", "text/plain")
        .header("x-proxy-error", blocked_header_value(reason))
        .body(Body::from(blocked_message(reason)))
        .unwrap_or_else(|_| Response::new(Body::from("blocked")))
}
