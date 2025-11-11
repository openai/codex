use std::path::PathBuf;
use std::sync::Arc;

use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::protocol::SessionSource;

use crate::ChatCompletionsApiClient;
use crate::ChatCompletionsApiClientConfig;
use crate::ResponseStream;
use crate::ResponsesApiClient;
use crate::ResponsesApiClientConfig;
use crate::Result;
use crate::WireApi;
use crate::WireEvent;
use crate::WireResponseStream;
use crate::api::PayloadClient;
use crate::auth::AuthProvider;
use crate::client::fixtures::stream_from_fixture;
use codex_provider_config::ModelProviderInfo;

/// Dispatches to the appropriate API client implementation based on the provider wire API.
#[derive(Clone)]
pub struct RoutedApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub model: String,
    pub conversation_id: ConversationId,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub otel_event_manager: OtelEventManager,
    pub session_source: SessionSource,
    pub responses_fixture_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct RoutedApiClient {
    config: RoutedApiClientConfig,
}

impl RoutedApiClient {
    pub fn new(config: RoutedApiClientConfig) -> Self {
        Self { config }
    }

    pub async fn stream_payload(&self, payload_json: &serde_json::Value) -> Result<ResponseStream> {
        match self.config.provider.wire_api {
            WireApi::Responses => {
                if let Some(path) = &self.config.responses_fixture_path {
                    return stream_from_fixture(
                        path,
                        self.config.provider.clone(),
                        self.config.otel_event_manager.clone(),
                    )
                    .await;
                }

                let cfg = ResponsesApiClientConfig {
                    http_client: self.config.http_client.clone(),
                    provider: self.config.provider.clone(),
                    model: self.config.model.clone(),
                    conversation_id: self.config.conversation_id,
                    auth_provider: self.config.auth_provider.clone(),
                    otel_event_manager: self.config.otel_event_manager.clone(),
                    extra_headers: vec![],
                };
                let client = <ResponsesApiClient as crate::api::PayloadClient>::new(cfg)?;
                client
                    .stream_payload(payload_json, Some(&self.config.session_source))
                    .await
            }
            WireApi::Chat => {
                let cfg = ChatCompletionsApiClientConfig {
                    http_client: self.config.http_client.clone(),
                    provider: self.config.provider.clone(),
                    model: self.config.model.clone(),
                    otel_event_manager: self.config.otel_event_manager.clone(),
                    session_source: self.config.session_source.clone(),
                    extra_headers: vec![],
                };
                let client = <ChatCompletionsApiClient as crate::api::PayloadClient>::new(cfg)?;
                client
                    .stream_payload(payload_json, Some(&self.config.session_source))
                    .await
            }
        }
    }

    pub async fn stream_payload_wire(
        &self,
        payload_json: &serde_json::Value,
    ) -> Result<WireResponseStream> {
        use futures::StreamExt;
        let legacy = self.stream_payload(payload_json).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1600);
        tokio::spawn(async move {
            futures::pin_mut!(legacy);
            while let Some(item) = legacy.next().await {
                let converted = item.and_then(|ev| map_response_event_to_wire(ev));
                if tx.send(converted).await.is_err() {
                    break;
                }
            }
        });
        Ok(crate::stream::EventStream::from_receiver(rx))
    }
}

#[async_trait::async_trait]
impl PayloadClient for RoutedApiClient {
    type Config = RoutedApiClientConfig;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Self::new(config))
    }

    async fn stream_payload(
        &self,
        payload_json: &serde_json::Value,
        _session_source: Option<&codex_protocol::protocol::SessionSource>,
    ) -> Result<ResponseStream> {
        self.stream_payload(payload_json).await
    }
}

fn map_response_event_to_wire(ev: crate::stream::ResponseEvent) -> Result<WireEvent> {
    crate::wire::map_response_event_to_wire(ev)
}
