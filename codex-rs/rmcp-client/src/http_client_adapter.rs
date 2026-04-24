//! RMCP Streamable HTTP adapter built on top of the shared `HttpClient`
//! capability.
//!
//! This module runs in the orchestrator process. It turns high-level RMCP
//! operations like `post_message` and `get_stream` into calls on
//! `Arc<dyn HttpClient>`, which may be:
//! - a local HTTP client that issues requests from the orchestrator, or
//! - a remote HTTP client that forwards requests to the remote runtime

use std::io;
use std::sync::Arc;

use bytes::Bytes;
use codex_api::SharedAuthProvider;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpResponseBodyStream;
use futures::StreamExt;
use futures::stream;
use futures::stream::BoxStream;
use reqwest::StatusCode;
use reqwest::header::ACCEPT;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::transport::streamable_http_client::AuthRequiredError;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use sse_stream::Sse;
use sse_stream::SseStream;

const EVENT_STREAM_MIME_TYPE: &str = "text/event-stream";
const JSON_MIME_TYPE: &str = "application/json";
const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const NON_JSON_RESPONSE_BODY_PREVIEW_BYTES: usize = 8_192;

#[derive(Clone)]
pub(crate) struct StreamableHttpClientAdapter {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
    auth_provider: Option<SharedAuthProvider>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StreamableHttpClientAdapterError {
    #[error("streamable HTTP session expired with 404 Not Found")]
    SessionExpired404,
    #[error(transparent)]
    HttpRequest(#[from] ExecServerError),
    #[error("invalid HTTP header: {0}")]
    Header(String),
}

impl StreamableHttpClientAdapter {
    pub(crate) fn new(
        http_client: Arc<dyn HttpClient>,
        default_headers: HeaderMap,
        auth_provider: Option<SharedAuthProvider>,
    ) -> Self {
        Self {
            http_client,
            default_headers,
            auth_provider,
        }
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
    ) -> std::result::Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let mut headers = self.default_headers.clone();
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

        let body = serde_json::to_vec(&message).map_err(StreamableHttpError::Deserialize)?;
        let (response, mut body_stream) = self
            .http_client
            .http_request_stream(HttpRequestParams {
                method: "POST".to_string(),
                url: uri.to_string(),
                headers: protocol_headers(&headers),
                body: Some(body.into()),
                timeout_ms: None,
                request_id: "buffered-request".to_string(),
                stream_response: true,
            })
            .await
            .map_err(StreamableHttpClientAdapterError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::NOT_FOUND.as_u16() && session_id.is_some() {
            return Err(StreamableHttpError::Client(
                StreamableHttpClientAdapterError::SessionExpired404,
            ));
        }
        if response.status == StatusCode::UNAUTHORIZED.as_u16()
            && let Some(header) =
                response_header(&response.headers, reqwest::header::WWW_AUTHENTICATE)
        {
            return Err(StreamableHttpError::AuthRequired(AuthRequiredError {
                www_authenticate_header: header,
            }));
        }
        if matches!(
            StatusCode::from_u16(response.status).ok(),
            Some(StatusCode::ACCEPTED | StatusCode::NO_CONTENT)
        ) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }

        let content_type = response_header(&response.headers, CONTENT_TYPE);
        let session_id = response_header(&response.headers, HEADER_SESSION_ID);
        match content_type.as_deref() {
            Some(content_type) if content_type.starts_with(EVENT_STREAM_MIME_TYPE) => {
                let event_stream = sse_stream_from_body(body_stream);
                Ok(StreamableHttpPostResponse::Sse(event_stream, session_id))
            }
            Some(content_type) if content_type.starts_with(JSON_MIME_TYPE) => {
                let body = collect_body(&mut body_stream).await?;
                let message: ServerJsonRpcMessage =
                    serde_json::from_slice(&body).map_err(StreamableHttpError::Deserialize)?;
                Ok(StreamableHttpPostResponse::Json(message, session_id))
            }
            _ => {
                let body = collect_body(&mut body_stream).await?;
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
    ) -> std::result::Result<(), StreamableHttpError<Self::Error>> {
        let mut headers = self.default_headers.clone();
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

        let response = self
            .http_client
            .http_request(HttpRequestParams {
                method: "DELETE".to_string(),
                url: uri.to_string(),
                headers: protocol_headers(&headers),
                body: None,
                timeout_ms: None,
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
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> std::result::Result<
        BoxStream<'static, std::result::Result<Sse, sse_stream::Error>>,
        StreamableHttpError<Self::Error>,
    > {
        let mut headers = self.default_headers.clone();
        self.add_auth_headers(&mut headers);
        insert_header(
            &mut headers,
            ACCEPT,
            [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "),
            StreamableHttpClientAdapterError::Header,
        )?;
        insert_header(
            &mut headers,
            HeaderName::from_static("mcp-session-id"),
            session_id.to_string(),
            StreamableHttpClientAdapterError::Header,
        )?;
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

        let (response, body_stream) = self
            .http_client
            .http_request_stream(HttpRequestParams {
                method: "GET".to_string(),
                url: uri.to_string(),
                headers: protocol_headers(&headers),
                body: None,
                timeout_ms: None,
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

        Ok(sse_stream_from_body(body_stream))
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

fn insert_header<Error>(
    headers: &mut HeaderMap,
    name: HeaderName,
    value: String,
    map_error: impl FnOnce(String) -> Error,
) -> std::result::Result<(), StreamableHttpError<Error>>
where
    Error: std::error::Error + Send + Sync + 'static,
{
    let value = reqwest::header::HeaderValue::from_str(&value)
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
                value: value.to_str().ok()?.to_string(),
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

async fn collect_body(
    body_stream: &mut HttpResponseBodyStream,
) -> std::result::Result<Vec<u8>, StreamableHttpError<StreamableHttpClientAdapterError>> {
    let mut body = Vec::new();
    while let Some(chunk) = body_stream
        .recv()
        .await
        .map_err(StreamableHttpClientAdapterError::from)
        .map_err(StreamableHttpError::Client)?
    {
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn sse_stream_from_body(
    body_stream: HttpResponseBodyStream,
) -> BoxStream<'static, std::result::Result<Sse, sse_stream::Error>> {
    SseStream::from_byte_stream(stream::unfold(body_stream, |mut body_stream| async move {
        match body_stream.recv().await {
            Ok(Some(bytes)) => Some((Ok(Bytes::from(bytes)), body_stream)),
            Ok(None) => None,
            Err(error) => Some((Err(io::Error::other(error)), body_stream)),
        }
    }))
    .boxed()
}
