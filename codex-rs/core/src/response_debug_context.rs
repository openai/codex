use base64::Engine;
use codex_api::TransportError;
use codex_api::error::ApiError;

const REQUEST_ID_HEADER: &str = "x-request-id";
const OAI_REQUEST_ID_HEADER: &str = "x-oai-request-id";
const CF_RAY_HEADER: &str = "cf-ray";
const AUTH_ERROR_HEADER: &str = "x-openai-authorization-error";
const X_ERROR_JSON_HEADER: &str = "x-error-json";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ResponseDebugContext {
    pub(crate) request_id: Option<String>,
    pub(crate) cf_ray: Option<String>,
    pub(crate) auth_error: Option<String>,
    pub(crate) auth_error_code: Option<String>,
}

pub(crate) fn extract_response_debug_context(transport: &TransportError) -> ResponseDebugContext {
    let mut context = ResponseDebugContext::default();

    let TransportError::Http {
        headers, body: _, ..
    } = transport
    else {
        return context;
    };

    let extract_header = |name: &str| {
        headers
            .as_ref()
            .and_then(|headers| headers.get(name))
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    };

    context.request_id =
        extract_header(REQUEST_ID_HEADER).or_else(|| extract_header(OAI_REQUEST_ID_HEADER));
    context.cf_ray = extract_header(CF_RAY_HEADER);
    context.auth_error = extract_header(AUTH_ERROR_HEADER);
    context.auth_error_code = extract_header(X_ERROR_JSON_HEADER).and_then(|encoded| {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .ok()?;
        let parsed = serde_json::from_slice::<serde_json::Value>(&decoded).ok()?;
        parsed
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });

    context
}

pub(crate) fn extract_response_debug_context_from_api_error(
    error: &ApiError,
) -> ResponseDebugContext {
    match error {
        ApiError::Transport(transport) => extract_response_debug_context(transport),
        _ => ResponseDebugContext::default(),
    }
}

pub(crate) fn telemetry_transport_error_message(error: &TransportError) -> String {
    match error {
        TransportError::Http { status, .. } => format!("http {}", status.as_u16()),
        TransportError::RetryLimit => "retry limit reached".to_string(),
        TransportError::Timeout => "timeout".to_string(),
        TransportError::Network(err) => err.to_string(),
        TransportError::Build(err) => err.to_string(),
    }
}

pub(crate) fn telemetry_api_error_message(error: &ApiError) -> String {
    match error {
        ApiError::Transport(transport) => telemetry_transport_error_message(transport),
        ApiError::Api { status, .. } => format!("api error {}", status.as_u16()),
        ApiError::Stream(err) => err.to_string(),
        ApiError::ContextWindowExceeded => "context window exceeded".to_string(),
        ApiError::QuotaExceeded => "quota exceeded".to_string(),
        ApiError::UsageNotIncluded => "usage not included".to_string(),
        ApiError::Retryable { .. } => "retryable error".to_string(),
        ApiError::RateLimit(_) => "rate limit".to_string(),
        ApiError::InvalidRequest { .. } => "invalid request".to_string(),
        ApiError::ServerOverloaded => "server overloaded".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::ResponseDebugContext;
    use super::extract_response_debug_context;
    use super::telemetry_api_error_message;
    use super::telemetry_transport_error_message;
    use codex_api::TransportError;
    use codex_api::error::ApiError;
    use http::HeaderMap;
    use http::HeaderValue;
    use http::StatusCode;
    use pretty_assertions::assert_eq;

    #[test]
    fn extract_response_debug_context_decodes_identity_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-oai-request-id", HeaderValue::from_static("req-auth"));
        headers.insert("cf-ray", HeaderValue::from_static("ray-auth"));
        headers.insert(
            "x-openai-authorization-error",
            HeaderValue::from_static("missing_authorization_header"),
        );
        headers.insert(
            "x-error-json",
            HeaderValue::from_static("eyJlcnJvciI6eyJjb2RlIjoidG9rZW5fZXhwaXJlZCJ9fQ=="),
        );

        let context = extract_response_debug_context(&TransportError::Http {
            status: StatusCode::UNAUTHORIZED,
            url: Some("https://chatgpt.com/backend-api/codex/models".to_string()),
            headers: Some(headers),
            body: Some(r#"{"error":{"message":"plain text error"},"status":401}"#.to_string()),
        });

        assert_eq!(
            context,
            ResponseDebugContext {
                request_id: Some("req-auth".to_string()),
                cf_ray: Some("ray-auth".to_string()),
                auth_error: Some("missing_authorization_header".to_string()),
                auth_error_code: Some("token_expired".to_string()),
            }
        );
    }

    #[test]
    fn telemetry_error_messages_omit_http_bodies() {
        let transport = TransportError::Http {
            status: StatusCode::UNAUTHORIZED,
            url: Some("https://chatgpt.com/backend-api/codex/responses".to_string()),
            headers: None,
            body: Some(r#"{"error":{"message":"secret token leaked"}}"#.to_string()),
        };

        assert_eq!(telemetry_transport_error_message(&transport), "http 401");
        assert_eq!(
            telemetry_api_error_message(&ApiError::Transport(transport)),
            "http 401"
        );
    }

    #[test]
    fn telemetry_error_messages_preserve_non_http_details() {
        let network = TransportError::Network("dns lookup failed".to_string());
        let build = TransportError::Build("invalid header value".to_string());
        let stream = ApiError::Stream("socket closed".to_string());

        assert_eq!(
            telemetry_transport_error_message(&network),
            "dns lookup failed"
        );
        assert_eq!(
            telemetry_transport_error_message(&build),
            "invalid header value"
        );
        assert_eq!(telemetry_api_error_message(&stream), "socket closed");
    }
}
