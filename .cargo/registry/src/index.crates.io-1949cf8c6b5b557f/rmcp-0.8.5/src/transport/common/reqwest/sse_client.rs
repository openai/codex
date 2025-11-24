use std::sync::Arc;

use futures::StreamExt;
use http::Uri;
use reqwest::header::ACCEPT;
use sse_stream::SseStream;

use crate::transport::{
    SseClientTransport,
    common::http_header::{EVENT_STREAM_MIME_TYPE, HEADER_LAST_EVENT_ID},
    sse_client::{SseClient, SseClientConfig, SseTransportError},
};

impl From<reqwest::Error> for SseTransportError<reqwest::Error> {
    fn from(e: reqwest::Error) -> Self {
        SseTransportError::Client(e)
    }
}

impl SseClient for reqwest::Client {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        uri: Uri,
        message: crate::model::ClientJsonRpcMessage,
        auth_token: Option<String>,
    ) -> Result<(), SseTransportError<Self::Error>> {
        let mut request_builder = self.post(uri.to_string()).json(&message);
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }
        request_builder
            .send()
            .await
            .and_then(|resp| resp.error_for_status())
            .map_err(SseTransportError::from)
            .map(drop)
    }

    async fn get_stream(
        &self,
        uri: Uri,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> Result<
        crate::transport::common::client_side_sse::BoxedSseResponse,
        SseTransportError<Self::Error>,
    > {
        let mut request_builder = self
            .get(uri.to_string())
            .header(ACCEPT, EVENT_STREAM_MIME_TYPE);
        if let Some(auth_header) = auth_token {
            request_builder = request_builder.bearer_auth(auth_header);
        }
        if let Some(last_event_id) = last_event_id {
            request_builder = request_builder.header(HEADER_LAST_EVENT_ID, last_event_id);
        }
        let response = request_builder.send().await?;
        let response = response.error_for_status()?;
        match response.headers().get(reqwest::header::CONTENT_TYPE) {
            Some(ct) => {
                if !ct.as_bytes().starts_with(EVENT_STREAM_MIME_TYPE.as_bytes()) {
                    return Err(SseTransportError::UnexpectedContentType(Some(
                        String::from_utf8_lossy(ct.as_bytes()).to_string(),
                    )));
                }
            }
            None => {
                return Err(SseTransportError::UnexpectedContentType(None));
            }
        }
        let event_stream = SseStream::from_byte_stream(response.bytes_stream()).boxed();
        Ok(event_stream)
    }
}

impl SseClientTransport<reqwest::Client> {
    /// Creates a new transport using reqwest with the specified SSE endpoint.
    ///
    /// This is a convenience method that creates a transport using the default
    /// reqwest client. This method is only available when the
    /// `transport-sse-client-reqwest` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `uri` - The SSE endpoint to connect to
    ///
    /// # Example
    ///
    /// ```rust
    /// use rmcp::transport::SseClientTransport;
    ///
    /// // Enable the reqwest feature in Cargo.toml:
    /// // rmcp = { version = "0.5", features = ["transport-sse-client-reqwest"] }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = SseClientTransport::start("http://localhost:8000/sse").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Feature requirement
    ///
    /// This method requires the `transport-sse-client-reqwest` feature.
    pub async fn start(
        uri: impl Into<Arc<str>>,
    ) -> Result<Self, SseTransportError<reqwest::Error>> {
        SseClientTransport::start_with_client(
            reqwest::Client::default(),
            SseClientConfig {
                sse_endpoint: uri.into(),
                ..Default::default()
            },
        )
        .await
    }
}
