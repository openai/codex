use rama::http::Body;
use rama::http::Response;
use rama::http::StatusCode;
use serde::Serialize;

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
        Err(_) => "{}".to_string(),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::from("{}")))
}

pub fn blocked_header_value(reason: &str) -> &'static str {
    match reason {
        "not_allowed" | "not_allowed_local" => "blocked-by-allowlist",
        "denied" => "blocked-by-denylist",
        "method_not_allowed" => "blocked-by-method-policy",
        "mitm_required" => "blocked-by-mitm-required",
        _ => "blocked-by-policy",
    }
}

pub fn blocked_message(reason: &str) -> &'static str {
    match reason {
        "not_allowed" => "Codex blocked this request: domain not in allowlist.",
        "not_allowed_local" => "Codex blocked this request: local/private addresses not allowed.",
        "denied" => "Codex blocked this request: domain denied by policy.",
        "method_not_allowed" => "Codex blocked this request: method not allowed in limited mode.",
        "mitm_required" => "Codex blocked this request: MITM required for limited HTTPS.",
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
