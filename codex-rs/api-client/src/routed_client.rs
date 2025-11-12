use std::path::PathBuf;
use std::sync::Arc;

use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;

use crate::ChatCompletionsApiClient;
use crate::ChatCompletionsApiClientConfig;
use crate::ResponsesApiClient;
use crate::ResponsesApiClientConfig;
use crate::Result;
use crate::WireApi;
use crate::WireResponseStream;
use crate::auth::AuthProvider;
use codex_provider_config::ModelProviderInfo;

/// Dispatches to the appropriate API client implementation based on the provider wire API.
#[derive(Clone)]
pub struct RoutedApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub conversation_id: ConversationId,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub otel_event_manager: OtelEventManager,
    pub responses_fixture_path: Option<PathBuf>,
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct RoutedApiClient {
    config: RoutedApiClientConfig,
}

impl RoutedApiClient {
    pub fn new(config: RoutedApiClientConfig) -> Self {
        Self { config }
    }

    pub async fn stream_payload_wire(
        &self,
        payload_json: &serde_json::Value,
    ) -> Result<WireResponseStream> {
        match self.config.provider.wire_api {
            WireApi::Responses => {
                let cfg = ResponsesApiClientConfig {
                    http_client: self.config.http_client.clone(),
                    provider: self.config.provider.clone(),
                    conversation_id: self.config.conversation_id,
                    auth_provider: self.config.auth_provider.clone(),
                    otel_event_manager: self.config.otel_event_manager.clone(),
                    extra_headers: self.config.extra_headers.clone(),
                };
                if let Some(path) = &self.config.responses_fixture_path {
                    return crate::client::fixtures::stream_from_fixture_wire(
                        path,
                        self.config.provider.clone(),
                        self.config.otel_event_manager.clone(),
                    )
                    .await;
                }
                let client = ResponsesApiClient::new(cfg)?;
                client.stream_payload_wire(payload_json).await
            }
            WireApi::Chat => {
                let cfg = ChatCompletionsApiClientConfig {
                    http_client: self.config.http_client.clone(),
                    provider: self.config.provider.clone(),
                    otel_event_manager: self.config.otel_event_manager.clone(),
                    extra_headers: self.config.extra_headers.clone(),
                };
                let client = ChatCompletionsApiClient::new(cfg)?;
                client.stream_payload_wire(payload_json).await
            }
        }
    }
}
