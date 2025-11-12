use crate::error::Error;
use crate::error::Result;
use crate::stream::WireResponseStream;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_provider_config::ModelProviderInfo;
use futures::TryStreamExt;

#[derive(Clone)]
/// Configuration for the Chat Completions client (OpenAI-compatible `/v1/chat/completions`).
///
/// - `http_client`: Reqwest client used for HTTP requests.
/// - `provider`: Provider configuration (base URL, headers, retries, etc.).
/// - `otel_event_manager`: Telemetry event manager for request/stream instrumentation.
pub struct ChatCompletionsApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub otel_event_manager: OtelEventManager,
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct ChatCompletionsApiClient {
    config: ChatCompletionsApiClientConfig,
}

impl ChatCompletionsApiClient {
    pub fn new(config: ChatCompletionsApiClientConfig) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn stream_payload_wire(
        &self,
        payload_json: &serde_json::Value,
    ) -> Result<WireResponseStream> {
        if self.config.provider.wire_api != codex_provider_config::WireApi::Chat {
            return Err(crate::error::Error::UnsupportedOperation(
                "ChatCompletionsApiClient requires a Chat provider".to_string(),
            ));
        }

        let extra_headers = crate::client::http::header_pairs(&self.config.extra_headers);
        let mut req_builder = crate::client::http::build_request(
            &self.config.http_client,
            &self.config.provider,
            &None,
            &extra_headers,
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

        let stream = res
            .bytes_stream()
            .map_err(|err| Error::ResponseStreamFailed {
                source: err,
                request_id: None,
            });
        let (_, rx_event) = crate::client::sse::spawn_wire_stream(
            stream,
            &self.config.provider,
            self.config.otel_event_manager.clone(),
            crate::decode_wire::chat::WireChatSseDecoder::new(),
        );

        Ok(rx_event)
    }
}
