//! RMCP Streamable HTTP adapter built on top of the shared `HttpClient`
//! capability.
//!
//! This module runs in the orchestrator process. It turns high-level RMCP
//! operations like `post_message` and `get_stream` into calls on
//! `Arc<dyn HttpClient>`, which may be:
//! - a local HTTP client that issues requests from the orchestrator, or
//! - a remote HTTP client that forwards requests to the remote runtime

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Instant;

use bytes::Bytes;
use codex_api::SharedAuthProvider;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpResponseBodyStream;
use futures::StreamExt;
use futures::stream;
use futures::stream::BoxStream;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::StatusCode;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use http::header::CONTENT_TYPE;
use http::header::WWW_AUTHENTICATE;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ClientNotification;
use rmcp::model::ConstString;
use rmcp::model::DiscoverRequestMethod;
use rmcp::model::ErrorCode;
use rmcp::model::ErrorData;
use rmcp::model::JsonRpcMessage;
use rmcp::model::ProtocolVersion;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::model::ServerResult;
use rmcp::transport::common::http_header::HEADER_MCP_PROTOCOL_VERSION;
use rmcp::transport::streamable_http_client::AuthRequiredError;
use rmcp::transport::streamable_http_client::InsufficientScopeError;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use sse_stream::Sse;
use sse_stream::SseStream;

use crate::incoming_jsonrpc::deserialize_incoming_jsonrpc_message;
use crate::incoming_jsonrpc::normalize_sse_jsonrpc_message;
use crate::local_stdio_transport::MAX_MCP_STDIO_LINE_BYTES;

mod www_authenticate;

use self::www_authenticate::insufficient_scope_challenge;

const EVENT_STREAM_MIME_TYPE: &str = "text/event-stream";
const JSON_MIME_TYPE: &str = "application/json";
const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const NON_JSON_RESPONSE_BODY_PREVIEW_BYTES: usize = 8_192;
const LEGACY_HTTP_PREVALIDATION_ERROR_CODE: ErrorCode = ErrorCode(-32000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamableHttpRedirectMode {
    Legacy,
    AgentPluginV1,
}

#[derive(Clone)]
pub(crate) struct StreamableHttpClientAdapter {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
    auth_provider: Option<SharedAuthProvider>,
    has_configured_headers: bool,
    redirect_mode: StreamableHttpRedirectMode,
    initialize_deadline: Arc<Mutex<Option<Instant>>>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StreamableHttpClientAdapterError {
    #[error("streamable HTTP session expired with 404 Not Found")]
    SessionExpired404,
    #[error(transparent)]
    HttpRequest(#[from] ExecServerError),
    #[error("invalid HTTP header: {0}")]
    Header(String),
    #[error("MCP response body exceeds {maximum_bytes} bytes")]
    ResponseTooLarge { maximum_bytes: usize },
}

impl StreamableHttpClientAdapter {
    pub(crate) fn new(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        auth_provider: Option<SharedAuthProvider>,
        has_configured_headers: bool,
        redirect_mode: StreamableHttpRedirectMode,
        initialize_deadline: Arc<Mutex<Option<Instant>>>,
    ) -> Self {
        Self {
            http_client,
            default_headers,
            auth_provider,
            has_configured_headers,
            redirect_mode,
            initialize_deadline,
        }
    }

    fn redirect_policy(&self, headers: &HeaderMap) -> HttpRedirectPolicy {
        mcp_redirect_policy(self.redirect_mode, headers, self.has_configured_headers)
    }
}

impl StreamableHttpClient for StreamableHttpClientAdapter {
    type Error = StreamableHttpClientAdapterError;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> std::result::Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let (mcp_method, mcp_request_id) = client_jsonrpc_message_fields(&message);
        let has_session_id = session_id.is_some();
        let mut headers = self.default_headers.clone();
        headers.extend(custom_headers);
        self.add_auth_headers(&mut headers);
        insert_header(
            &mut headers,
            ACCEPT,
            [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "),
            StreamableHttpClientAdapterError::Header,
        )?;
        insert_header(
            &mut headers,
            CONTENT_TYPE,
            JSON_MIME_TYPE.to_string(),
            StreamableHttpClientAdapterError::Header,
        )?;
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                StreamableHttpClientAdapterError::Header,
            )?;
        }
        if let Some(session_id_value) = session_id.as_ref() {
            insert_header(
                &mut headers,
                HeaderName::from_static("mcp-session-id"),
                session_id_value.to_string(),
                StreamableHttpClientAdapterError::Header,
            )?;
        }

        let maximum_response_bytes = (mcp_method.as_deref() == Some(DiscoverRequestMethod::VALUE)
            || headers
                .get(HEADER_MCP_PROTOCOL_VERSION)
                .and_then(|value| value.to_str().ok())
                == Some(ProtocolVersion::V_2026_07_28.as_str()))
        .then_some(MAX_MCP_STDIO_LINE_BYTES);
        let redirect_policy = if mcp_method.as_deref() == Some(DiscoverRequestMethod::VALUE) {
            HttpRedirectPolicy::Stop
        } else {
            self.redirect_policy(&headers)
        };
        let timeout_ms = if matches!(
            mcp_method.as_deref(),
            Some("initialize" | "notifications/initialized")
        ) || mcp_method.as_deref() == Some(DiscoverRequestMethod::VALUE)
        {
            self.initialize_deadline
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .map(|deadline| {
                    u64::try_from(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis(),
                    )
                    .unwrap_or(u64::MAX)
                    .max(1)
                })
        } else {
            None
        };

        let body = serde_json::to_vec(&message).map_err(StreamableHttpError::Deserialize)?;
        let has_authorization_header = headers.contains_key(AUTHORIZATION);
        let response = self
            .http_client
            .http_request_stream(HttpRequestParams {
                method: "POST".to_string(),
                url: uri.to_string(),
                headers: protocol_headers(&headers),
                body: Some(body.into()),
                timeout_ms,
                redirect_policy,
                request_id: "buffered-request".to_string(),
                stream_response: true,
            })
            .await;
        let (response, mut body_stream) = match response {
            Ok(response) => response,
            Err(error) => {
                log_post_message_http_error(
                    &uri,
                    mcp_method.as_deref(),
                    mcp_request_id.as_deref(),
                    has_session_id,
                    has_authorization_header,
                );
                return Err(StreamableHttpError::Client(
                    StreamableHttpClientAdapterError::from(error),
                ));
            }
        };

        if response.status == StatusCode::NOT_FOUND.as_u16() && session_id.is_some() {
            return Err(StreamableHttpError::Client(
                StreamableHttpClientAdapterError::SessionExpired404,
            ));
        }
        if response.status == StatusCode::UNAUTHORIZED.as_u16()
            && let Some(header) = response_header(&response.headers, WWW_AUTHENTICATE)
        {
            return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(
                header,
            )));
        }
        if response.status == StatusCode::FORBIDDEN.as_u16()
            && let Some(challenge) = insufficient_scope_challenge(&response.headers)
        {
            return Err(StreamableHttpError::InsufficientScope(
                InsufficientScopeError::new(
                    challenge.www_authenticate_header,
                    challenge.required_scope,
                ),
            ));
        }
        if matches!(
            StatusCode::from_u16(response.status).ok(),
            Some(StatusCode::ACCEPTED | StatusCode::NO_CONTENT)
        ) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }

        let content_type = response_header(&response.headers, CONTENT_TYPE);
        let session_id = response_header(&response.headers, HEADER_SESSION_ID);
        if !status_is_success(response.status) {
            let body = collect_body(&mut body_stream, maximum_response_bytes).await?;
            if !retryable_post_response_status(mcp_method.as_deref(), response.status)
                && (content_type
                    .as_deref()
                    .is_some_and(|content_type| content_type.starts_with(JSON_MIME_TYPE))
                    || (mcp_method.as_deref() == Some(DiscoverRequestMethod::VALUE)
                        && response.status == StatusCode::BAD_REQUEST.as_u16()
                        && !has_session_id))
                && let Some(response_message) = parse_json_rpc_error(&body)
            {
                return Ok(StreamableHttpPostResponse::Json(
                    legacy_discovery_fallback_response(
                        &message,
                        response_message,
                        response.status == StatusCode::BAD_REQUEST.as_u16() && !has_session_id,
                    ),
                    session_id,
                ));
            }
            if mcp_method.as_deref() == Some(DiscoverRequestMethod::VALUE)
                && !has_session_id
                && matches!(
                    StatusCode::from_u16(response.status).ok(),
                    Some(StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED)
                )
                && content_type
                    .as_deref()
                    .is_none_or(|content_type| !content_type.starts_with(JSON_MIME_TYPE))
                && let JsonRpcMessage::Request(request) = &message
            {
                let legacy_error = ServerJsonRpcMessage::error(
                    ErrorData::new(
                        ErrorCode::METHOD_NOT_FOUND,
                        "legacy MCP endpoint does not support server/discover",
                        None,
                    ),
                    Some(request.id.clone()),
                );
                return Ok(StreamableHttpPostResponse::Json(legacy_error, session_id));
            }
            return Err(StreamableHttpError::UnexpectedServerResponse(
                format!(
                    "HTTP {}: {}",
                    response.status,
                    body_preview(String::from_utf8_lossy(&body).to_string())
                )
                .into(),
            ));
        }
        match content_type.as_deref() {
            Some(content_type) if content_type.starts_with(EVENT_STREAM_MIME_TYPE) => {
                let mut event_stream = sse_stream_from_body(body_stream, maximum_response_bytes);
                if mcp_method.as_deref() == Some(DiscoverRequestMethod::VALUE) {
                    while let Some(event) = event_stream.next().await {
                        let event = event.map_err(StreamableHttpError::Sse)?;
                        if !matches!(event.event.as_deref(), None | Some("") | Some("message")) {
                            continue;
                        }
                        let Some(data) = event.data.as_deref() else {
                            continue;
                        };
                        if data.trim().is_empty() {
                            continue;
                        }

                        let response = deserialize_incoming_jsonrpc_message(data.as_bytes())
                            .map_err(StreamableHttpError::Deserialize)?;
                        let response = legacy_discovery_fallback_response(
                            &message, response, /*allow_uncorrelated_http_rejection*/ false,
                        );
                        if matches!(
                            &response,
                            JsonRpcMessage::Response(_) | JsonRpcMessage::Error(_)
                        ) {
                            return Ok(StreamableHttpPostResponse::Json(response, session_id));
                        }
                    }

                    return Err(StreamableHttpError::UnexpectedServerResponse(
                        "empty sse stream".into(),
                    ));
                }
                Ok(StreamableHttpPostResponse::Sse(event_stream, session_id))
            }
            Some(content_type) if content_type.starts_with(JSON_MIME_TYPE) => {
                let body = collect_body(&mut body_stream, maximum_response_bytes).await?;
                let response_message = deserialize_incoming_jsonrpc_message(&body)
                    .map_err(StreamableHttpError::Deserialize)?;
                Ok(StreamableHttpPostResponse::Json(
                    legacy_discovery_fallback_response(
                        &message,
                        response_message,
                        /*allow_uncorrelated_http_rejection*/ false,
                    ),
                    session_id,
                ))
            }
            _ => {
                let body = collect_body(&mut body_stream, maximum_response_bytes).await?;
                let content_type = content_type.unwrap_or_else(|| "missing-content-type".into());
                Err(StreamableHttpError::UnexpectedContentType(Some(format!(
                    "{content_type}; body: {}",
                    body_preview(String::from_utf8_lossy(&body).to_string())
                ))))
            }
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> std::result::Result<(), StreamableHttpError<Self::Error>> {
        let mut headers = self.default_headers.clone();
        headers.extend(custom_headers);
        self.add_auth_headers(&mut headers);
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                StreamableHttpClientAdapterError::Header,
            )?;
        }
        insert_header(
            &mut headers,
            HeaderName::from_static("mcp-session-id"),
            session.to_string(),
            StreamableHttpClientAdapterError::Header,
        )?;
        let redirect_policy = self.redirect_policy(&headers);

        let response = self
            .http_client
            .http_request(HttpRequestParams {
                method: "DELETE".to_string(),
                url: uri.to_string(),
                headers: protocol_headers(&headers),
                body: None,
                timeout_ms: None,
                redirect_policy,
                request_id: "buffered-request".to_string(),
                stream_response: false,
            })
            .await
            .map_err(StreamableHttpClientAdapterError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::METHOD_NOT_ALLOWED.as_u16() {
            return Ok(());
        }
        if !status_is_success(response.status) {
            return Err(StreamableHttpError::UnexpectedServerResponse(
                format!("DELETE returned HTTP {}", response.status).into(),
            ));
        }
        Ok(())
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> std::result::Result<
        BoxStream<'static, std::result::Result<Sse, sse_stream::Error>>,
        StreamableHttpError<Self::Error>,
    > {
        let mut headers = self.default_headers.clone();
        headers.extend(custom_headers);
        self.add_auth_headers(&mut headers);
        insert_header(
            &mut headers,
            ACCEPT,
            [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "),
            StreamableHttpClientAdapterError::Header,
        )?;
        if let Some(session_id) = session_id {
            insert_header(
                &mut headers,
                HeaderName::from_static("mcp-session-id"),
                session_id.to_string(),
                StreamableHttpClientAdapterError::Header,
            )?;
        }
        if let Some(last_event_id) = last_event_id {
            insert_header(
                &mut headers,
                HeaderName::from_static("last-event-id"),
                last_event_id,
                StreamableHttpClientAdapterError::Header,
            )?;
        }
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                StreamableHttpClientAdapterError::Header,
            )?;
        }
        let redirect_policy = self.redirect_policy(&headers);

        let (response, body_stream) = self
            .http_client
            .http_request_stream(HttpRequestParams {
                method: "GET".to_string(),
                url: uri.to_string(),
                headers: protocol_headers(&headers),
                body: None,
                timeout_ms: None,
                redirect_policy,
                request_id: "buffered-request".to_string(),
                stream_response: true,
            })
            .await
            .map_err(StreamableHttpClientAdapterError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::METHOD_NOT_ALLOWED.as_u16() {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        if response.status == StatusCode::NOT_FOUND.as_u16() {
            return Err(StreamableHttpError::Client(
                StreamableHttpClientAdapterError::SessionExpired404,
            ));
        }
        if !status_is_success(response.status) {
            return Err(StreamableHttpError::UnexpectedServerResponse(
                format!("GET returned HTTP {}", response.status).into(),
            ));
        }

        match response_header(&response.headers, CONTENT_TYPE).as_deref() {
            Some(content_type) if is_streamable_http_content_type(content_type) => {}
            Some(content_type) => {
                return Err(StreamableHttpError::UnexpectedContentType(Some(
                    content_type.to_string(),
                )));
            }
            None => {
                return Err(StreamableHttpError::UnexpectedContentType(None));
            }
        }

        let maximum_response_bytes = headers
            .get(HEADER_MCP_PROTOCOL_VERSION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|version| version == ProtocolVersion::V_2026_07_28.as_str())
            .then_some(MAX_MCP_STDIO_LINE_BYTES);
        Ok(sse_stream_from_body(body_stream, maximum_response_bytes))
    }
}

impl StreamableHttpClientAdapter {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        if let Some(auth_provider) = &self.auth_provider {
            headers.extend(auth_provider.to_auth_headers());
        }
    }
}

fn body_preview(body: impl Into<String>) -> String {
    let mut body_preview = body.into();
    let body_len = body_preview.len();
    if body_len > NON_JSON_RESPONSE_BODY_PREVIEW_BYTES {
        let mut boundary = NON_JSON_RESPONSE_BODY_PREVIEW_BYTES;
        while !body_preview.is_char_boundary(boundary) {
            boundary = boundary.saturating_sub(1);
        }
        body_preview.truncate(boundary);
        body_preview.push_str(&format!(
            "... (truncated {} bytes)",
            body_len.saturating_sub(boundary)
        ));
    }
    body_preview
}

fn client_jsonrpc_message_fields(
    message: &ClientJsonRpcMessage,
) -> (Option<String>, Option<String>) {
    match message {
        JsonRpcMessage::Request(request) => (
            Some(request.request.method().to_string()),
            Some(request.id.to_string()),
        ),
        JsonRpcMessage::Response(response) => (None, Some(response.id.to_string())),
        JsonRpcMessage::Notification(notification) => {
            let method = match &notification.notification {
                ClientNotification::CancelledNotification(notification) => {
                    notification.method.as_str()
                }
                ClientNotification::ProgressNotification(notification) => {
                    notification.method.as_str()
                }
                ClientNotification::InitializedNotification(notification) => {
                    notification.method.as_str()
                }
                ClientNotification::RootsListChangedNotification(notification) => {
                    notification.method.as_str()
                }
                ClientNotification::CustomNotification(notification) => {
                    notification.method.as_str()
                }
                _ => return (None, None),
            };
            (Some(method.to_string()), None)
        }
        JsonRpcMessage::Error(error) => (None, error.id.as_ref().map(ToString::to_string)),
    }
}

fn log_post_message_http_error(
    uri: &str,
    mcp_method: Option<&str>,
    mcp_request_id: Option<&str>,
    has_session_id: bool,
    has_authorization_header: bool,
) {
    let parsed_url = url::Url::parse(uri).ok();
    tracing::warn!(
        endpoint_scheme = parsed_url
            .as_ref()
            .map(url::Url::scheme)
            .unwrap_or("<invalid>"),
        endpoint_host = parsed_url
            .as_ref()
            .and_then(url::Url::host_str)
            .unwrap_or("<invalid>"),
        endpoint_path = parsed_url
            .as_ref()
            .map(url::Url::path)
            .unwrap_or("<invalid>"),
        endpoint_has_query = parsed_url.as_ref().is_some_and(|url| url.query().is_some()),
        mcp_method = mcp_method.unwrap_or("<none>"),
        mcp_request_id = mcp_request_id.unwrap_or("<none>"),
        has_session_id = has_session_id,
        has_authorization_header = has_authorization_header,
        "streamable HTTP post_message failed"
    );
}

fn insert_header<Error>(
    headers: &mut HeaderMap,
    name: HeaderName,
    value: String,
    map_error: impl FnOnce(String) -> Error,
) -> std::result::Result<(), StreamableHttpError<Error>>
where
    Error: std::error::Error + Send + Sync + 'static,
{
    let value = HeaderValue::from_str(&value)
        .map_err(|error| StreamableHttpError::Client(map_error(error.to_string())))?;
    headers.insert(name, value);
    Ok(())
}

fn is_streamable_http_content_type(content_type: &str) -> bool {
    content_type
        .as_bytes()
        .starts_with(EVENT_STREAM_MIME_TYPE.as_bytes())
        || content_type
            .as_bytes()
            .starts_with(JSON_MIME_TYPE.as_bytes())
}

fn protocol_headers(headers: &HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some(HttpHeader {
                name: name.as_str().to_string(),
                value: std::str::from_utf8(value.as_bytes()).ok()?.to_string(),
            })
        })
        .collect()
}

fn response_header(headers: &[HttpHeader], name: impl AsRef<str>) -> Option<String> {
    let name = name.as_ref();
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.clone())
}

fn status_is_success(status: u16) -> bool {
    StatusCode::from_u16(status).is_ok_and(|status| status.is_success())
}

fn retryable_post_response_status(mcp_method: Option<&str>, status: u16) -> bool {
    let Ok(status) = StatusCode::from_u16(status) else {
        return false;
    };
    is_retryable_http_status(status)
        && matches!(
            mcp_method,
            Some(
                DiscoverRequestMethod::VALUE
                    | "initialize"
                    | "notifications/initialized"
                    | "tools/list"
            )
        )
}

fn is_retryable_http_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn parse_json_rpc_error(body: &[u8]) -> Option<ServerJsonRpcMessage> {
    match serde_json::from_slice::<ServerJsonRpcMessage>(body) {
        Ok(message @ JsonRpcMessage::Error(_)) => Some(message),
        _ => None,
    }
}

fn mcp_redirect_policy(
    mode: StreamableHttpRedirectMode,
    headers: &HeaderMap,
    has_configured_headers: bool,
) -> HttpRedirectPolicy {
    if headers
        .get(HEADER_MCP_PROTOCOL_VERSION)
        .and_then(|value| value.to_str().ok())
        == Some(ProtocolVersion::V_2026_07_28.as_str())
        || (mode == StreamableHttpRedirectMode::AgentPluginV1
            && (has_configured_headers || headers.contains_key(AUTHORIZATION)))
    {
        HttpRedirectPolicy::Stop
    } else {
        HttpRedirectPolicy::Follow
    }
}

// rmcp's automatic lifecycle does not yet recognize deployed legacy discovery
// rejection shapes. Remove this compatibility shim once the SDK does:
// https://github.com/modelcontextprotocol/rust-sdk/issues/1040
fn legacy_discovery_fallback_response(
    request: &ClientJsonRpcMessage,
    response: ServerJsonRpcMessage,
    allow_uncorrelated_http_rejection: bool,
) -> ServerJsonRpcMessage {
    let JsonRpcMessage::Request(request) = request else {
        return response;
    };
    if request.request.method() != DiscoverRequestMethod::VALUE {
        return response;
    }

    if let JsonRpcMessage::Error(error) = &response
        && error.error.code == ErrorCode::METHOD_NOT_FOUND
        && error.id.as_ref() != Some(&request.id)
    {
        return ServerJsonRpcMessage::error(
            ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                "server/discover method-not-found response did not match its request ID",
                None,
            ),
            Some(request.id.clone()),
        );
    }

    let requires_legacy_initialization = match &response {
        JsonRpcMessage::Response(response) if response.id == request.id => match &response.result {
            ServerResult::DiscoverResult(result) => {
                only_known_legacy_protocol_versions(&result.supported_versions)
            }
            _ => false,
        },
        JsonRpcMessage::Error(error) if error.id.as_ref() == Some(&request.id) => {
            (error.error.code == ErrorCode::UNSUPPORTED_PROTOCOL_VERSION
                && error
                    .error
                    .data
                    .as_ref()
                    .and_then(|data| data.get("supported"))
                    .and_then(|supported| {
                        serde_json::from_value::<Vec<ProtocolVersion>>(supported.clone()).ok()
                    })
                    .is_some_and(|supported| only_known_legacy_protocol_versions(&supported)))
                || matches!(
                    error.error.code,
                    ErrorCode::UNSUPPORTED_PROTOCOL_VERSION
                        | ErrorCode::INVALID_REQUEST
                        | ErrorCode::INVALID_PARAMS
                ) && explicitly_rejects_modern_protocol_version(&error.error.message)
        }
        JsonRpcMessage::Error(error)
            if allow_uncorrelated_http_rejection
                && error.id.is_none()
                && error.error.code == LEGACY_HTTP_PREVALIDATION_ERROR_CODE =>
        {
            has_legacy_fallback_evidence(&error.error.message)
        }
        _ => false,
    };

    if requires_legacy_initialization {
        ServerJsonRpcMessage::error(
            ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "MCP discovery requires legacy initialization",
                None,
            ),
            Some(request.id.clone()),
        )
    } else {
        response
    }
}

fn only_known_legacy_protocol_versions(versions: &[ProtocolVersion]) -> bool {
    !versions.is_empty()
        && versions.iter().all(|version| {
            ProtocolVersion::KNOWN_VERSIONS.contains(version)
                && version < &ProtocolVersion::V_2026_07_28
        })
}

fn explicitly_rejects_modern_protocol_version(message: &str) -> bool {
    message
        .trim()
        .eq_ignore_ascii_case("unsupported protocol version: 2026-07-28")
}

// Some legacy servers reject `server/discover` before assigning a JSON-RPC ID.
// A null-ID HTTP 400/-32000 does not, by itself, justify a downgrade.
// Retry `initialize` only for the exact missing-session error or a list of
// exclusively legacy versions that includes a version rmcp supports.
// These are compatibility hints, not proof of server identity; `initialize`
// negotiates the actual version, and 2025-06-18 is only our initial proposal.
fn has_legacy_fallback_evidence(message: &str) -> bool {
    if message == "Bad Request: No valid session ID provided" {
        return true;
    }

    let Some(supported) = message
        .strip_prefix("Bad Request: Unsupported protocol version: 2026-07-28 (supported versions: ")
        .or_else(|| {
            message.strip_prefix("Bad Request: Unsupported protocol version (supported versions: ")
        })
        .and_then(|supported| supported.strip_suffix(')'))
    else {
        return false;
    };

    let versions = supported.split(',').map(str::trim).collect::<Vec<_>>();
    !versions.is_empty()
        && ProtocolVersion::KNOWN_VERSIONS
            .iter()
            .any(|known| versions.contains(&known.as_str()))
        && versions.iter().all(|version| {
            let bytes = version.as_bytes();
            bytes.len() == 10
                && bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes
                    .iter()
                    .enumerate()
                    .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
                && *version < "2026-07-28"
        })
}

async fn collect_body(
    body_stream: &mut HttpResponseBodyStream,
    maximum_bytes: Option<usize>,
) -> std::result::Result<Vec<u8>, StreamableHttpError<StreamableHttpClientAdapterError>> {
    let mut body = Vec::new();
    while let Some(chunk) = body_stream
        .recv()
        .await
        .map_err(StreamableHttpClientAdapterError::from)
        .map_err(StreamableHttpError::Client)?
    {
        if let Some(maximum_bytes) = maximum_bytes
            && chunk.len() > maximum_bytes.saturating_sub(body.len())
        {
            return Err(StreamableHttpError::Client(
                StreamableHttpClientAdapterError::ResponseTooLarge { maximum_bytes },
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn sse_stream_from_body(
    body_stream: HttpResponseBodyStream,
    maximum_event_bytes: Option<usize>,
) -> BoxStream<'static, std::result::Result<Sse, sse_stream::Error>> {
    let modern_session = maximum_event_bytes.is_some();
    SseStream::from_bytes_stream(stream::unfold(
        (body_stream, SseEventSizeLimit::new(maximum_event_bytes)),
        |(mut body_stream, mut size_limit)| async move {
            match body_stream.recv().await {
                Ok(Some(bytes)) => {
                    if let Err(error) = size_limit.observe(&bytes) {
                        Some((Err(error), (body_stream, size_limit)))
                    } else {
                        Some((Ok(Bytes::from(bytes)), (body_stream, size_limit)))
                    }
                }
                Ok(None) => None,
                Err(error) => Some((Err(io::Error::other(error)), (body_stream, size_limit))),
            }
        },
    ))
    .map(move |event| {
        event.map(|mut event| {
            if let Some(payload) = event.data.as_deref()
                && let Some(normalized) = normalize_sse_jsonrpc_message(payload, modern_session)
            {
                event.data = Some(normalized);
            }
            event
        })
    })
    .boxed()
}

struct SseEventSizeLimit {
    maximum_bytes: Option<usize>,
    retained_bytes: usize,
    line_bytes: usize,
    line_is_comment: bool,
    previous_was_carriage_return: bool,
    failed: bool,
}

impl SseEventSizeLimit {
    fn new(maximum_bytes: Option<usize>) -> Self {
        Self {
            maximum_bytes,
            retained_bytes: 0,
            line_bytes: 0,
            line_is_comment: false,
            previous_was_carriage_return: false,
            failed: false,
        }
    }

    fn observe(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized MCP SSE event was already rejected",
            ));
        }
        let Some(maximum_bytes) = self.maximum_bytes else {
            return Ok(());
        };

        for &byte in bytes {
            if self.previous_was_carriage_return {
                self.previous_was_carriage_return = false;
                if byte == b'\n' {
                    continue;
                }
            }

            match byte {
                b'\r' => {
                    self.finish_line(maximum_bytes)?;
                    self.previous_was_carriage_return = true;
                }
                b'\n' => self.finish_line(maximum_bytes)?,
                _ => {
                    if self.line_bytes == 0 {
                        self.line_is_comment = byte == b':';
                    }
                    self.line_bytes = self.line_bytes.saturating_add(1);
                    self.check_limit(maximum_bytes)?;
                }
            }
        }
        Ok(())
    }

    fn finish_line(&mut self, maximum_bytes: usize) -> io::Result<()> {
        if self.line_bytes == 0 {
            self.retained_bytes = 0;
        } else if !self.line_is_comment {
            // The SSE parser inserts a newline when joining multiple data fields.
            self.retained_bytes = self
                .retained_bytes
                .saturating_add(self.line_bytes)
                .saturating_add(1);
        }

        self.line_bytes = 0;
        self.line_is_comment = false;
        self.check_limit(maximum_bytes)
    }

    fn check_limit(&mut self, maximum_bytes: usize) -> io::Result<()> {
        if self.retained_bytes.saturating_add(self.line_bytes) > maximum_bytes {
            self.failed = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("MCP response body exceeds {maximum_bytes} bytes"),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "http_client_adapter_tests.rs"]
mod tests;
