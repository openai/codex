//! Reference: <https://html.spec.whatwg.org/multipage/server-sent-events.html>
use std::{
    pin::Pin,
    sync::{Arc, RwLock},
};

use futures::{StreamExt, future::BoxFuture};
use http::Uri;
use sse_stream::{Error as SseError, Sse};
use thiserror::Error;

use super::{
    Transport,
    common::client_side_sse::{BoxedSseResponse, SseRetryPolicy, SseStreamReconnect},
};
use crate::{
    RoleClient,
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::common::client_side_sse::SseAutoReconnectStream,
};

#[derive(Error, Debug)]
pub enum SseTransportError<E: std::error::Error + Send + Sync + 'static> {
    #[error("SSE error: {0}")]
    Sse(#[from] SseError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Client error: {0}")]
    Client(E),
    #[error("unexpected end of stream")]
    UnexpectedEndOfStream,
    #[error("Unexpected content type: {0:?}")]
    UnexpectedContentType(Option<String>),
    #[cfg(feature = "auth")]
    #[cfg_attr(docsrs, doc(cfg(feature = "auth")))]
    #[error("Auth error: {0}")]
    Auth(#[from] crate::transport::auth::AuthError),
    #[error("Invalid uri: {0}")]
    InvalidUri(#[from] http::uri::InvalidUri),
    #[error("Invalid uri parts: {0}")]
    InvalidUriParts(#[from] http::uri::InvalidUriParts),
}

pub trait SseClient: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;
    fn post_message(
        &self,
        uri: Uri,
        message: ClientJsonRpcMessage,
        auth_token: Option<String>,
    ) -> impl Future<Output = Result<(), SseTransportError<Self::Error>>> + Send + '_;
    fn get_stream(
        &self,
        uri: Uri,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> impl Future<Output = Result<BoxedSseResponse, SseTransportError<Self::Error>>> + Send + '_;
}

/// Helper that refreshes the POST endpoint whenever the server emits
/// control frames during SSE reconnect; used together with
/// [`SseAutoReconnectStream`].
struct SseClientReconnect<C> {
    pub client: C,
    pub uri: Uri,
    pub message_endpoint: Arc<RwLock<Uri>>,
}

impl<C: SseClient> SseStreamReconnect for SseClientReconnect<C> {
    type Error = SseTransportError<C::Error>;
    type Future = BoxFuture<'static, Result<BoxedSseResponse, Self::Error>>;
    fn retry_connection(&mut self, last_event_id: Option<&str>) -> Self::Future {
        let client = self.client.clone();
        let uri = self.uri.clone();
        let last_event_id = last_event_id.map(|s| s.to_owned());
        Box::pin(async move { client.get_stream(uri, last_event_id, None).await })
    }

    fn handle_control_event(&mut self, event: &Sse) -> Result<(), Self::Error> {
        if event.event.as_deref() != Some("endpoint") {
            return Ok(());
        }
        let Some(data) = event.data.as_ref() else {
            return Ok(());
        };
        // Servers typically resend the message POST endpoint (often with a new
        // sessionId) when a stream reconnects. Reuse `message_endpoint` helper
        // to resolve it and update the shared URI.
        let new_endpoint = message_endpoint(self.uri.clone(), data.clone())
            .map_err(SseTransportError::InvalidUri)?;
        *self
            .message_endpoint
            .write()
            .expect("message endpoint lock poisoned") = new_endpoint;
        Ok(())
    }

    fn handle_stream_error(
        &mut self,
        error: &(dyn std::error::Error + 'static),
        last_event_id: Option<&str>,
    ) {
        tracing::warn!(
            uri = %self.uri,
            last_event_id = last_event_id.unwrap_or(""),
            "sse stream error: {error}"
        );
    }
}
type ServerMessageStream<C> = Pin<Box<SseAutoReconnectStream<SseClientReconnect<C>>>>;

/// A client-agnostic SSE transport for RMCP that supports Server-Sent Events.
///
/// This transport allows you to choose your preferred HTTP client implementation
/// by implementing the [`SseClient`] trait. The transport handles SSE streaming
/// and automatic reconnection.
///
/// # Usage
///
/// ## Using reqwest
///
/// ```rust,ignore
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
/// ## Using a custom HTTP client
///
/// ```rust,ignore
/// use rmcp::transport::sse_client::{SseClient, SseClientTransport, SseClientConfig};
/// use std::sync::Arc;
/// use futures::stream::BoxStream;
/// use rmcp::model::ClientJsonRpcMessage;
/// use sse_stream::{Sse, Error as SseError};
/// use http::Uri;
///
/// #[derive(Clone)]
/// struct MyHttpClient;
///
/// #[derive(Debug, thiserror::Error)]
/// struct MyError;
///
/// impl std::fmt::Display for MyError {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         write!(f, "MyError")
///     }
/// }
///
/// impl SseClient for MyHttpClient {
///     type Error = MyError;
///     
///     async fn post_message(
///         &self,
///         _uri: Uri,
///         _message: ClientJsonRpcMessage,
///         _auth_token: Option<String>,
///     ) -> Result<(), rmcp::transport::sse_client::SseTransportError<Self::Error>> {
///         todo!()
///     }
///     
///     async fn get_stream(
///         &self,
///         _uri: Uri,
///         _last_event_id: Option<String>,
///         _auth_token: Option<String>,
///     ) -> Result<BoxStream<'static, Result<Sse, SseError>>, rmcp::transport::sse_client::SseTransportError<Self::Error>> {
///         todo!()
///     }
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = SseClientConfig {
///     sse_endpoint: "http://localhost:8000/sse".into(),
///     ..Default::default()
/// };
/// let transport = SseClientTransport::start_with_client(MyHttpClient, config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Feature Flags
///
/// - `transport-sse-client`: Base feature providing the generic transport infrastructure
/// - `transport-sse-client-reqwest`: Includes reqwest HTTP client support with convenience methods
pub struct SseClientTransport<C: SseClient> {
    client: C,
    config: SseClientConfig,
    /// Current POST endpoint; refreshed when the server sends new endpoint
    /// control frames.
    message_endpoint: Arc<RwLock<Uri>>,
    stream: Option<ServerMessageStream<C>>,
}

impl<C: SseClient> Transport<RoleClient> for SseClientTransport<C> {
    type Error = SseTransportError<C::Error>;
    async fn receive(&mut self) -> Option<ServerJsonRpcMessage> {
        self.stream.as_mut()?.next().await?.ok()
    }
    fn send(
        &mut self,
        item: crate::service::TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let client = self.client.clone();
        let message_endpoint = self.message_endpoint.clone();
        async move {
            let uri = {
                let guard = message_endpoint
                    .read()
                    .expect("message endpoint lock poisoned");
                guard.clone()
            };
            client.post_message(uri, item, None).await
        }
    }
    async fn close(&mut self) -> Result<(), Self::Error> {
        self.stream.take();
        Ok(())
    }
}

impl<C: SseClient + std::fmt::Debug> std::fmt::Debug for SseClientTransport<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SseClientWorker")
            .field("client", &self.client)
            .field("config", &self.config)
            .finish()
    }
}

impl<C: SseClient> SseClientTransport<C> {
    pub async fn start_with_client(
        client: C,
        config: SseClientConfig,
    ) -> Result<Self, SseTransportError<C::Error>> {
        let sse_endpoint = config.sse_endpoint.as_ref().parse::<http::Uri>()?;

        let mut sse_stream = client.get_stream(sse_endpoint.clone(), None, None).await?;
        let initial_message_endpoint = if let Some(endpoint) = config.use_message_endpoint.clone() {
            let ep = endpoint.parse::<http::Uri>()?;
            let mut sse_endpoint_parts = sse_endpoint.clone().into_parts();
            sse_endpoint_parts.path_and_query = ep.into_parts().path_and_query;
            Uri::from_parts(sse_endpoint_parts)?
        } else {
            // wait the endpoint event
            loop {
                let sse = sse_stream
                    .next()
                    .await
                    .ok_or(SseTransportError::UnexpectedEndOfStream)??;
                let Some("endpoint") = sse.event.as_deref() else {
                    continue;
                };
                let ep = sse.data.unwrap_or_default();

                break message_endpoint(sse_endpoint.clone(), ep)?;
            }
        };
        let message_endpoint = Arc::new(RwLock::new(initial_message_endpoint));

        let stream = Box::pin(SseAutoReconnectStream::new(
            sse_stream,
            SseClientReconnect {
                client: client.clone(),
                uri: sse_endpoint.clone(),
                message_endpoint: message_endpoint.clone(),
            },
            config.retry_policy.clone(),
        ));
        Ok(Self {
            client,
            config,
            message_endpoint,
            stream: Some(stream),
        })
    }
}

fn message_endpoint(base: http::Uri, endpoint: String) -> Result<http::Uri, http::uri::InvalidUri> {
    // If endpoint is a full URL, parse and return it directly
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.parse::<http::Uri>();
    }

    let mut base_parts = base.into_parts();
    let endpoint_clone = endpoint.clone();

    if endpoint.starts_with("?") {
        // Query only - keep base path and append query
        if let Some(base_path_and_query) = &base_parts.path_and_query {
            let base_path = base_path_and_query.path();
            base_parts.path_and_query = Some(format!("{}{}", base_path, endpoint).parse()?);
        } else {
            base_parts.path_and_query = Some(format!("/{}", endpoint).parse()?);
        }
    } else {
        // Path (with optional query) - replace entire path_and_query
        let path_to_use = if endpoint.starts_with("/") {
            endpoint // Use absolute path as-is
        } else {
            format!("/{}", endpoint) // Make relative path absolute
        };
        base_parts.path_and_query = Some(path_to_use.parse()?);
    }

    http::Uri::from_parts(base_parts).map_err(|_| endpoint_clone.parse::<http::Uri>().unwrap_err())
}

#[derive(Debug, Clone)]
pub struct SseClientConfig {
    /// client sse endpoint
    ///
    /// # How this client resolve the message endpoint
    /// if sse_endpoint has this format: `<schema><authority?><sse_pq>`,
    /// then the message endpoint will be `<schema><authority?><message_pq>`.
    ///
    /// For example, if you config the sse_endpoint as `http://example.com/some_path/sse`,
    /// and the server send the message endpoint event as `message?session_id=123`,
    /// then the message endpoint will be `http://example.com/message`.
    ///
    /// This follows the rules of JavaScript's [`new URL(url, base)`](https://developer.mozilla.org/en-US/docs/Web/API/URL/URL)
    pub sse_endpoint: Arc<str>,
    pub retry_policy: Arc<dyn SseRetryPolicy>,
    /// if this is settled, the client will use this endpoint to send message and skip get the endpoint event
    pub use_message_endpoint: Option<String>,
}

impl Default for SseClientConfig {
    fn default() -> Self {
        Self {
            sse_endpoint: "".into(),
            retry_policy: Arc::new(super::common::client_side_sse::FixedInterval::default()),
            use_message_endpoint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use serde_json::{Value, json};

    use super::*;

    #[derive(Clone)]
    struct DummyClient;

    #[derive(Debug, thiserror::Error)]
    #[error("dummy error")]
    struct DummyError;

    impl SseClient for DummyClient {
        type Error = DummyError;

        async fn post_message(
            &self,
            _uri: Uri,
            _message: ClientJsonRpcMessage,
            _auth_token: Option<String>,
        ) -> Result<(), SseTransportError<Self::Error>> {
            Ok(())
        }

        async fn get_stream(
            &self,
            _uri: Uri,
            _last_event_id: Option<String>,
            _auth_token: Option<String>,
        ) -> Result<BoxedSseResponse, SseTransportError<Self::Error>> {
            unreachable!("get_stream should not be called in this test")
        }
    }

    #[test]
    fn test_message_endpoint() {
        let base_url = "https://localhost/sse".parse::<http::Uri>().unwrap();

        // Query only
        let result = message_endpoint(base_url.clone(), "?sessionId=x".to_string()).unwrap();
        assert_eq!(result.to_string(), "https://localhost/sse?sessionId=x");

        // Relative path with query
        let result = message_endpoint(base_url.clone(), "mypath?sessionId=x".to_string()).unwrap();
        assert_eq!(result.to_string(), "https://localhost/mypath?sessionId=x");

        // Absolute path with query
        let result = message_endpoint(base_url.clone(), "/xxx?sessionId=x".to_string()).unwrap();
        assert_eq!(result.to_string(), "https://localhost/xxx?sessionId=x");

        // Full URL
        let result = message_endpoint(
            base_url.clone(),
            "http://example.com/xxx?sessionId=x".to_string(),
        )
        .unwrap();
        assert_eq!(result.to_string(), "http://example.com/xxx?sessionId=x");
    }

    #[test]
    fn handle_endpoint_control_event_updates_uri() {
        let initial_endpoint = "https://example.com/message?sessionId=old"
            .parse::<Uri>()
            .unwrap();
        let shared_endpoint = Arc::new(RwLock::new(initial_endpoint));
        let mut reconnect = SseClientReconnect {
            client: DummyClient,
            uri: "https://example.com/sse".parse::<Uri>().unwrap(),
            message_endpoint: shared_endpoint.clone(),
        };

        let control_event = Sse::default()
            .event("endpoint")
            .data("/message?sessionId=new");

        reconnect.handle_control_event(&control_event).unwrap();

        let guard = shared_endpoint.read().expect("lock poisoned");
        assert_eq!(
            guard.to_string(),
            "https://example.com/message?sessionId=new"
        );
    }

    #[tokio::test]
    async fn control_event_frames_are_skipped() {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"ok": true}
        })
        .to_string();

        let events = vec![
            Ok(Sse::default()
                .event("endpoint")
                .data("/message?sessionId=reconnect")),
            Ok(Sse::default().event("message").data(payload.clone())),
        ];

        let sse_src: BoxedSseResponse = futures::stream::iter(events).boxed();
        let reconn_stream = SseAutoReconnectStream::never_reconnect(sse_src, DummyError);
        futures::pin_mut!(reconn_stream);

        let message = reconn_stream.next().await.expect("stream item").unwrap();
        let actual: Value = serde_json::to_value(message).expect("serialize actual message");
        // We only need to assert that a valid JSON-RPC response came through after
        // skipping control frames. The exact `result` shape depends on the SDK's
        // typed result enums and is not asserted here.
        assert_eq!(actual.get("jsonrpc"), Some(&Value::String("2.0".into())));
        assert_eq!(actual.get("id"), Some(&Value::Number(1u64.into())));
    }
}
