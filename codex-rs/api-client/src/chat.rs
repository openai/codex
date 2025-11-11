use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::protocol::SessionSource;
use futures::TryStreamExt;
use tokio::sync::mpsc;

use crate::api::PayloadClient;
use crate::error::Error;
use crate::error::Result;
use crate::stream::ResponseEvent;
use crate::stream::ResponseStream;
use codex_provider_config::ModelProviderInfo;

#[derive(Clone)]
/// Configuration for the Chat Completions client (OpenAI-compatible `/v1/chat/completions`).
///
/// - `http_client`: Reqwest client used for HTTP requests.
/// - `provider`: Provider configuration (base URL, headers, retries, etc.).
/// - `model`: Model identifier to use.
/// - `otel_event_manager`: Telemetry event manager for request/stream instrumentation.
/// - `session_source`: Session metadata, used to set subagent headers when applicable.
pub struct ChatCompletionsApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub model: String,
    pub otel_event_manager: OtelEventManager,
    pub session_source: SessionSource,
}

#[derive(Clone)]
pub struct ChatCompletionsApiClient {
    config: ChatCompletionsApiClientConfig,
}

// prompt-based API removed; use PayloadClient::stream_payload instead

// prompt-based API removed

#[async_trait]
impl PayloadClient for ChatCompletionsApiClient {
    type Config = ChatCompletionsApiClientConfig;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Self { config })
    }

    async fn stream_payload(
        &self,
        payload_json: &serde_json::Value,
        session_source: Option<&codex_protocol::protocol::SessionSource>,
    ) -> Result<ResponseStream> {
        if self.config.provider.wire_api != codex_provider_config::WireApi::Chat {
            return Err(crate::error::Error::UnsupportedOperation(
                "ChatCompletionsApiClient requires a Chat provider".to_string(),
            ));
        }

        let auth = crate::client::http::resolve_auth(&None).await;
        let mut req_builder = crate::client::http::build_request(
            &self.config.http_client,
            &self.config.provider,
            &auth,
            session_source,
            &[],
        )
        .await?;

        req_builder = req_builder
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(payload_json);

        let res = self
            .config
            .otel_event_manager
            .log_request(0, || req_builder.send())
            .await?;

        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
        let stream = res
            .bytes_stream()
            .map_err(|err| Error::ResponseStreamFailed {
                source: err,
                request_id: None,
            });
        let idle_timeout = self.config.provider.stream_idle_timeout();
        let otel = self.config.otel_event_manager.clone();
        tokio::spawn(crate::client::sse::process_sse(
            stream,
            tx_event,
            idle_timeout,
            otel,
            crate::decode::chat::ChatSseDecoder::new(),
        ));

        Ok(crate::stream::EventStream::from_receiver(rx_event))
    }
}
