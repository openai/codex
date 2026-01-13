use crate::auth::AuthProvider;
use crate::common::Prompt as ApiPrompt;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::endpoint::responses::ResponsesOptions;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::requests::ResponsesRequest;
use crate::requests::ResponsesRequestBuilder;
use crate::requests::responses::Compression;
use crate::sse::responses::ResponsesStreamEvent;
use crate::sse::responses::process_responses_event;
use codex_client::TransportError;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use http::HeaderValue;
use serde_json::Value;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::debug;
use tracing::trace;
use tracing::warn;
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct ResponsesWebsocketClient<A: AuthProvider> {
    provider: Provider,
    auth: A,
}

impl<A: AuthProvider> ResponsesWebsocketClient<A> {
    pub fn new(provider: Provider, auth: A) -> Self {
        Self { provider, auth }
    }

    pub async fn stream_request(
        &self,
        request: ResponsesRequest,
    ) -> Result<ResponseStream, ApiError> {
        self.stream(request.body, request.headers, request.compression)
            .await
    }

    pub async fn stream_prompt(
        &self,
        model: &str,
        prompt: &ApiPrompt,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let ResponsesOptions {
            reasoning,
            include,
            prompt_cache_key,
            text,
            store_override,
            conversation_id,
            session_source,
            extra_headers,
            compression,
        } = options;

        // TODO (pakrym): share with HTTP based Responses API client
        let request = ResponsesRequestBuilder::new(model, &prompt.instructions, &prompt.input)
            .tools(&prompt.tools)
            .parallel_tool_calls(prompt.parallel_tool_calls)
            .reasoning(reasoning)
            .include(include)
            .prompt_cache_key(prompt_cache_key)
            .text(text)
            .conversation(conversation_id)
            .session_source(session_source)
            .store_override(store_override)
            .extra_headers(extra_headers)
            .compression(compression)
            .build(&self.provider)?;

        self.stream_request(request).await
    }

    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
        compression: Compression,
    ) -> Result<ResponseStream, ApiError> {
        if compression == Compression::Zstd {
            warn!(
                "request compression is not supported for websocket streaming; sending uncompressed payload"
            );
        }

        let ws_url = Url::parse(&self.provider.url_for_path("responses"))
            .map_err(|err| ApiError::Stream(format!("failed to build websocket URL: {err}")))?;
        let mut headers = self.provider.headers.clone();
        headers.extend(extra_headers);
        apply_auth_headers(&mut headers, &self.auth);

        let connection = connect_websocket(ws_url, headers).await?;

        let (tx_event, rx_event) =
            mpsc::channel::<std::result::Result<ResponseEvent, ApiError>>(1600);
        let idle_timeout = self.provider.stream_idle_timeout;

        // TODO (pakrym): surface rate limits
        // TODO (pakrym): check models etags

        tokio::spawn(async move {
            if let Err(err) = run_websocket_response_stream(
                connection.stream,
                tx_event.clone(),
                body,
                idle_timeout,
            )
            .await
            {
                let _ = tx_event.send(Err(err)).await;
            }
        });

        Ok(ResponseStream { rx_event })
    }
}

// TODO (pakrym): share with /auth
fn apply_auth_headers(headers: &mut HeaderMap, auth: &impl AuthProvider) {
    if let Some(token) = auth.bearer_token()
        && let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        let _ = headers.insert(http::header::AUTHORIZATION, header);
    }
    if let Some(account_id) = auth.account_id()
        && let Ok(header) = HeaderValue::from_str(&account_id)
    {
        let _ = headers.insert("ChatGPT-Account-ID", header);
    }
}

struct WebSocketConnection {
    stream: WsStream,
}

async fn connect_websocket(url: Url, headers: HeaderMap) -> Result<WebSocketConnection, ApiError> {
    let mut request = url
        .clone()
        .into_client_request()
        .map_err(|err| ApiError::Stream(format!("failed to build websocket request: {err}")))?;
    request.headers_mut().extend(headers);

    let (stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|err| map_ws_error(err, &url))?;
    Ok(WebSocketConnection { stream })
}

fn map_ws_error(err: WsError, url: &Url) -> ApiError {
    match err {
        WsError::Http(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .body()
                .as_ref()
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok());
            ApiError::Transport(TransportError::Http {
                status,
                url: Some(url.to_string()),
                headers: Some(headers),
                body,
            })
        }
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            ApiError::Stream("websocket closed".to_string())
        }
        WsError::Io(err) => ApiError::Transport(TransportError::Network(err.to_string())),
        other => ApiError::Transport(TransportError::Network(other.to_string())),
    }
}

async fn run_websocket_response_stream(
    mut ws_stream: WsStream,
    tx_event: mpsc::Sender<std::result::Result<ResponseEvent, ApiError>>,
    request_body: Value,
    idle_timeout: Duration,
) -> Result<(), ApiError> {
    let request_text = match serde_json::to_string(&request_body) {
        Ok(text) => text,
        Err(err) => {
            let _ = ws_stream.close(None).await;
            return Err(ApiError::Stream(format!(
                "failed to encode websocket request: {err}"
            )));
        }
    };

    if let Err(err) = ws_stream.send(Message::Text(request_text)).await {
        let _ = ws_stream.close(None).await;
        return Err(ApiError::Stream(format!(
            "failed to send websocket request: {err}"
        )));
    }

    loop {
        let response = tokio::time::timeout(idle_timeout, ws_stream.next())
            .await
            .map_err(|_| ApiError::Stream("idle timeout waiting for websocket".into()));
        let message = match response {
            Ok(Some(Ok(msg))) => msg,
            Ok(Some(Err(err))) => {
                let _ = ws_stream.close(None).await;
                return Err(ApiError::Stream(err.to_string()));
            }
            Ok(None) => {
                let _ = ws_stream.close(None).await;
                return Err(ApiError::Stream(
                    "stream closed before response.completed".into(),
                ));
            }
            Err(err) => {
                let _ = ws_stream.close(None).await;
                return Err(err);
            }
        };

        match message {
            Message::Text(text) => {
                trace!("websocket event: {text}");
                let event = match serde_json::from_str::<ResponsesStreamEvent>(&text) {
                    Ok(event) => event,
                    Err(err) => {
                        debug!("failed to parse websocket event: {err}, data: {text}");
                        continue;
                    }
                };
                match process_responses_event(event) {
                    Ok(Some(event)) => {
                        let is_completed = matches!(event, ResponseEvent::Completed { .. });
                        let _ = tx_event.send(Ok(event)).await;
                        if is_completed {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = ws_stream.close(None).await;
                        return Err(error.into_api_error());
                    }
                }
            }
            Message::Binary(_) => {
                let _ = ws_stream.close(None).await;
                return Err(ApiError::Stream("unexpected binary websocket event".into()));
            }
            Message::Ping(payload) => {
                if ws_stream.send(Message::Pong(payload)).await.is_err() {
                    let _ = ws_stream.close(None).await;
                    return Err(ApiError::Stream("websocket ping failed".into()));
                }
            }
            Message::Pong(_) => {}
            Message::Close(_) => {
                let _ = ws_stream.close(None).await;
                return Err(ApiError::Stream(
                    "websocket closed before response.completed".into(),
                ));
            }
            _ => {}
        }
    }

    let _ = ws_stream.close(None).await;
    Ok(())
}
