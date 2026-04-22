use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use codex_client::build_reqwest_client_with_custom_ca;
use futures::StreamExt;
use futures::stream::BoxStream;
use reqwest::header::ACCEPT;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::WWW_AUTHENTICATE;
use rmcp::transport::streamable_http_client::AuthRequiredError;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use sse_stream::Sse;
use sse_stream::SseStream;

use crate::streamable_http_transport_client::StreamableHttpTransportClientError;
use crate::utils::apply_default_headers;

const EVENT_STREAM_MIME_TYPE: &str = "text/event-stream";
const JSON_MIME_TYPE: &str = "application/json";
const HEADER_LAST_EVENT_ID: &str = "Last-Event-Id";
const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const NON_JSON_RESPONSE_BODY_PREVIEW_BYTES: usize = 8_192;

#[derive(Clone)]
pub(crate) struct LocalStreamableHttpClient {
    inner: reqwest::Client,
}

impl LocalStreamableHttpClient {
    pub(crate) fn new(default_headers: HeaderMap) -> Result<Self> {
        let builder = apply_default_headers(reqwest::Client::builder(), &default_headers);
        let inner = build_reqwest_client_with_custom_ca(builder)?;
        Ok(Self { inner })
    }
}

impl StreamableHttpClient for LocalStreamableHttpClient {
    type Error = StreamableHttpTransportClientError;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: rmcp::model::ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
    ) -> std::result::Result<
        StreamableHttpPostResponse,
        StreamableHttpError<StreamableHttpTransportClientError>,
    > {
        let mut request = self
            .inner
            .post(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "));
        if let Some(auth_header) = auth_token {
            request = request.bearer_auth(auth_header);
        }
        if let Some(session_id_value) = session_id.as_ref() {
            request = request.header(HEADER_SESSION_ID, session_id_value.as_ref());
        }

        let response = request
            .json(&message)
            .send()
            .await
            .map_err(StreamableHttpTransportClientError::from)
            .map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND && session_id.is_some() {
            return Err(StreamableHttpError::Client(
                StreamableHttpTransportClientError::SessionExpired404,
            ));
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let Some(header) = response.headers().get(WWW_AUTHENTICATE)
        {
            let header = header
                .to_str()
                .map_err(|_| {
                    StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(
                        "invalid www-authenticate header value",
                    ))
                })?
                .to_string();
            return Err(StreamableHttpError::AuthRequired(AuthRequiredError {
                www_authenticate_header: header,
            }));
        }

        let status = response.status();
        if matches!(
            status,
            reqwest::StatusCode::ACCEPTED | reqwest::StatusCode::NO_CONTENT
        ) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let session_id = response
            .headers()
            .get(HEADER_SESSION_ID)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        match content_type.as_deref() {
            Some(ct) if ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes()) => {
                let event_stream = SseStream::from_byte_stream(response.bytes_stream()).boxed();
                Ok(StreamableHttpPostResponse::Sse(event_stream, session_id))
            }
            Some(ct) if ct.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes()) => {
                let message = response
                    .json()
                    .await
                    .map_err(StreamableHttpTransportClientError::from)
                    .map_err(StreamableHttpError::Client)?;
                Ok(StreamableHttpPostResponse::Json(message, session_id))
            }
            _ => {
                let body = response
                    .text()
                    .await
                    .map_err(StreamableHttpTransportClientError::from)
                    .map_err(StreamableHttpError::Client)?;
                let mut body_preview = body;
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

                let content_type = content_type.unwrap_or_else(|| "missing-content-type".into());
                Err(StreamableHttpError::UnexpectedContentType(Some(format!(
                    "{content_type}; body: {body_preview}"
                ))))
            }
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
    ) -> std::result::Result<(), StreamableHttpError<StreamableHttpTransportClientError>> {
        let mut request_builder = self.inner.delete(uri.as_ref());
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }
        let response = request_builder
            .header(HEADER_SESSION_ID, session.as_ref())
            .send()
            .await
            .map_err(StreamableHttpTransportClientError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }

        response
            .error_for_status()
            .map_err(StreamableHttpTransportClientError::from)
            .map_err(StreamableHttpError::Client)?;
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
        StreamableHttpError<StreamableHttpTransportClientError>,
    > {
        let mut request_builder = self
            .inner
            .get(uri.as_ref())
            .header(ACCEPT, [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "))
            .header(HEADER_SESSION_ID, session_id.as_ref());
        if let Some(last_event_id) = last_event_id {
            request_builder = request_builder.header(HEADER_LAST_EVENT_ID, last_event_id);
        }
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }

        let response = request_builder
            .send()
            .await
            .map_err(StreamableHttpTransportClientError::from)
            .map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StreamableHttpError::Client(
                StreamableHttpTransportClientError::SessionExpired404,
            ));
        }

        let response = response
            .error_for_status()
            .map_err(StreamableHttpTransportClientError::from)
            .map_err(StreamableHttpError::Client)?;
        match response.headers().get(CONTENT_TYPE) {
            Some(ct)
                if ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes())
                    || ct.as_bytes().starts_with(JSON_MIME_TYPE.as_bytes()) => {}
            Some(ct) => {
                return Err(StreamableHttpError::UnexpectedContentType(Some(
                    String::from_utf8_lossy(ct.as_bytes()).to_string(),
                )));
            }
            None => {
                return Err(StreamableHttpError::UnexpectedContentType(None));
            }
        }

        let event_stream = SseStream::from_byte_stream(response.bytes_stream()).boxed();
        Ok(event_stream)
    }
}
