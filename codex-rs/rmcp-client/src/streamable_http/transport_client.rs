use std::sync::Arc;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
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

use crate::streamable_http::common::EVENT_STREAM_MIME_TYPE;
use crate::streamable_http::common::HEADER_SESSION_ID;
use crate::streamable_http::common::JSON_MIME_TYPE;
use crate::streamable_http::common::body_preview;
use crate::streamable_http::common::insert_header;
use crate::streamable_http::common::is_streamable_http_content_type;

mod response;

use response::collect_body;
use response::protocol_headers;
use response::response_header;
use response::sse_stream_from_body;
use response::status_is_success;

#[derive(Clone)]
pub(crate) struct HttpBackedStreamableHttpClient {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum HttpBackedStreamableHttpClientError {
    #[error("streamable HTTP session expired with 404 Not Found")]
    SessionExpired404,
    #[error(transparent)]
    HttpRequest(#[from] ExecServerError),
    #[error("invalid HTTP header: {0}")]
    Header(String),
}

impl HttpBackedStreamableHttpClient {
    pub(crate) fn new(http_client: Arc<dyn HttpClient>, default_headers: HeaderMap) -> Self {
        Self {
            http_client,
            default_headers,
        }
    }
}

impl StreamableHttpClient for HttpBackedStreamableHttpClient {
    type Error = HttpBackedStreamableHttpClientError;

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
            HttpBackedStreamableHttpClientError::Header,
        )?;
        insert_header(
            &mut headers,
            CONTENT_TYPE,
            JSON_MIME_TYPE.to_string(),
            HttpBackedStreamableHttpClientError::Header,
        )?;
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                HttpBackedStreamableHttpClientError::Header,
            )?;
        }
        if let Some(session_id_value) = session_id.as_ref() {
            insert_header(
                &mut headers,
                HeaderName::from_static("mcp-session-id"),
                session_id_value.to_string(),
                HttpBackedStreamableHttpClientError::Header,
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
            .map_err(HttpBackedStreamableHttpClientError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::NOT_FOUND.as_u16() && session_id.is_some() {
            return Err(StreamableHttpError::Client(
                HttpBackedStreamableHttpClientError::SessionExpired404,
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
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                HttpBackedStreamableHttpClientError::Header,
            )?;
        }
        insert_header(
            &mut headers,
            HeaderName::from_static("mcp-session-id"),
            session.to_string(),
            HttpBackedStreamableHttpClientError::Header,
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
            .map_err(HttpBackedStreamableHttpClientError::from)
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
        insert_header(
            &mut headers,
            ACCEPT,
            [EVENT_STREAM_MIME_TYPE, JSON_MIME_TYPE].join(", "),
            HttpBackedStreamableHttpClientError::Header,
        )?;
        insert_header(
            &mut headers,
            HeaderName::from_static("mcp-session-id"),
            session_id.to_string(),
            HttpBackedStreamableHttpClientError::Header,
        )?;
        if let Some(last_event_id) = last_event_id {
            insert_header(
                &mut headers,
                HeaderName::from_static("last-event-id"),
                last_event_id,
                HttpBackedStreamableHttpClientError::Header,
            )?;
        }
        if let Some(auth_token) = auth_token {
            insert_header(
                &mut headers,
                AUTHORIZATION,
                format!("Bearer {auth_token}"),
                HttpBackedStreamableHttpClientError::Header,
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
            .map_err(HttpBackedStreamableHttpClientError::from)
            .map_err(StreamableHttpError::Client)?;

        if response.status == StatusCode::METHOD_NOT_ALLOWED.as_u16() {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        if response.status == StatusCode::NOT_FOUND.as_u16() {
            return Err(StreamableHttpError::Client(
                HttpBackedStreamableHttpClientError::SessionExpired404,
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
