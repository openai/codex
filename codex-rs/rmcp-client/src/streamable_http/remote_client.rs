//! Streamable HTTP transport that performs network requests through exec-server.
//!
//! RMCP still owns the MCP protocol state machine, session recovery, and OAuth
//! decisions. This adapter only translates RMCP's HTTP operations into the
//! executor HTTP request protocol so remote executors resolve network
//! addresses, headers, and streaming bodies from the executor side.

use std::io;
use std::sync::Arc;

use bytes::Bytes;
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
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use sse_stream::Sse;
use sse_stream::SseStream;

use crate::streamable_http::common::EVENT_STREAM_MIME_TYPE;
use crate::streamable_http::common::HEADER_SESSION_ID;
use crate::streamable_http::common::JSON_MIME_TYPE;
use crate::streamable_http::common::body_preview;
use crate::streamable_http::common::insert_header;
use crate::streamable_http::common::is_streamable_http_content_type;
use crate::streamable_http::remote_client::RemoteStreamableHttpClientError::Header;

/// RMCP Streamable HTTP client that sends HTTP requests through exec-server.
///
/// The client is deliberately small: it translates HTTP operations to
/// executor protocol calls and lets RMCP own MCP session and recovery behavior.
#[derive(Clone)]
pub(crate) struct RemoteStreamableHttpClient {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
}

/// Errors introduced by executor-backed Streamable HTTP transport.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RemoteStreamableHttpClientError {
    /// Existing MCP session id was rejected with 404.
    #[error("streamable HTTP session expired with 404 Not Found")]
    SessionExpired404,
    /// The executor HTTP request failed before producing an RMCP response.
    #[error(transparent)]
    ExecServer(#[from] ExecServerError),
    /// Header value construction failed before sending the executor request.
    #[error("invalid HTTP header: {0}")]
    Header(String),
}

impl RemoteStreamableHttpClient {
    /// Creates an adapter with shared executor client and static default headers.
    pub(crate) fn new(http_client: Arc<dyn HttpClient>, default_headers: HeaderMap) -> Self {
        Self {
            http_client,
            default_headers,
        }
    }
}

impl StreamableHttpClient for RemoteStreamableHttpClient {
    type Error = RemoteStreamableHttpClientError;

    /// Sends a JSON-RPC message to the MCP server over executor HTTP.
    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
    ) -> std::result::Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let mut headers = self.default_headers.clone();
        insert_header(
            &mut headers,
            ACCEPT,
            [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "),
            Header,
        )?;
        insert_header(
            &mut headers,
            CONTENT_TYPE,
            JSON_MIME_TYPE.to_string(),
            Header,
        )?;
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                Header,
            )?;
        }
        if let Some(session_id_value) = session_id.as_ref() {
            insert_header(
                &mut headers,
                HeaderName::from_static("mcp-session-id"),
                session_id_value.to_string(),
                Header,
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
            .map_err(RemoteStreamableHttpClientError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::NOT_FOUND.as_u16() && session_id.is_some() {
            return Err(StreamableHttpError::Client(
                RemoteStreamableHttpClientError::SessionExpired404,
            ));
        }
        if response.status == StatusCode::UNAUTHORIZED.as_u16()
            && let Some(header) =
                response_header(&response.headers, reqwest::header::WWW_AUTHENTICATE)
        {
            return Err(StreamableHttpError::AuthRequired(
                rmcp::transport::streamable_http_client::AuthRequiredError {
                    www_authenticate_header: header,
                },
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
        match content_type.as_deref() {
            Some(content_type) if content_type.starts_with(EVENT_STREAM_MIME_TYPE) => {
                let event_stream = sse_stream_from_body(body_stream).boxed();
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

    /// Deletes an MCP Streamable HTTP session through executor HTTP.
    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
    ) -> std::result::Result<(), StreamableHttpError<Self::Error>> {
        let mut headers = self.default_headers.clone();
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                Header,
            )?;
        }
        insert_header(
            &mut headers,
            HeaderName::from_static("mcp-session-id"),
            session.to_string(),
            Header,
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
            .map_err(RemoteStreamableHttpClientError::from)
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

    /// Opens a server stream through executor HTTP and exposes it as SSE bytes.
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
        insert_header(
            &mut headers,
            ACCEPT,
            [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "),
            Header,
        )?;
        insert_header(
            &mut headers,
            HeaderName::from_static("mcp-session-id"),
            session_id.to_string(),
            Header,
        )?;
        if let Some(last_event_id) = last_event_id {
            insert_header(
                &mut headers,
                HeaderName::from_static("last-event-id"),
                last_event_id,
                Header,
            )?;
        }
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                Header,
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
            .map_err(RemoteStreamableHttpClientError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::METHOD_NOT_ALLOWED.as_u16() {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        if response.status == StatusCode::NOT_FOUND.as_u16() {
            return Err(StreamableHttpError::Client(
                RemoteStreamableHttpClientError::SessionExpired404,
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

        Ok(sse_stream_from_body(body_stream).boxed())
    }
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
) -> std::result::Result<Vec<u8>, StreamableHttpError<RemoteStreamableHttpClientError>> {
    let mut body = Vec::new();
    while let Some(chunk) = body_stream
        .recv()
        .await
        .map_err(RemoteStreamableHttpClientError::from)
        .map_err(StreamableHttpError::Client)?
    {
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn sse_stream_from_body(
    body_stream: HttpResponseBodyStream,
) -> BoxStream<'static, std::result::Result<Sse, sse_stream::Error>> {
    let byte_stream = stream::unfold(Some(body_stream), |state| async move {
        let mut body_stream = state?;
        match body_stream.recv().await {
            Ok(Some(bytes)) => Some((Ok(Bytes::from(bytes)), Some(body_stream))),
            Ok(None) => None,
            Err(error) => Some((Err(io::Error::other(error)), None::<HttpResponseBodyStream>)),
        }
    });
    SseStream::from_byte_stream(byte_stream).boxed()
}
