use std::path::PathBuf;
use std::sync::Arc;

use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::protocol::SessionSource;

use crate::ApiClient;
use crate::ChatAggregationMode;
use crate::ChatCompletionsApiClient;
use crate::ChatCompletionsApiClientConfig;
use crate::Prompt;
use crate::ResponseStream;
use crate::ResponsesApiClient;
use crate::ResponsesApiClientConfig;
use crate::Result;
use crate::WireApi;
use crate::auth::AuthProvider;
use crate::model_provider::ModelProviderInfo;
use crate::responses::stream_from_fixture;

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
    pub chat_aggregation_mode: ChatAggregationMode,
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

    pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        match self.config.provider.wire_api {
            WireApi::Responses => self.stream_responses(prompt).await,
            WireApi::Chat => self.stream_chat(prompt).await,
        }
    }

    async fn stream_responses(&self, prompt: &Prompt) -> Result<ResponseStream> {
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
        };
        let client = ResponsesApiClient::new(cfg)?;
        client.stream(prompt).await
    }

    async fn stream_chat(&self, prompt: &Prompt) -> Result<ResponseStream> {
        let cfg = ChatCompletionsApiClientConfig {
            http_client: self.config.http_client.clone(),
            provider: self.config.provider.clone(),
            model: self.config.model.clone(),
            otel_event_manager: self.config.otel_event_manager.clone(),
            session_source: self.config.session_source.clone(),
            aggregation_mode: self.config.chat_aggregation_mode,
        };
        let client = ChatCompletionsApiClient::new(cfg)?;
        client.stream(prompt).await
    }
}

#[async_trait::async_trait]
impl ApiClient for RoutedApiClient {
    type Config = RoutedApiClientConfig;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Self::new(config))
    }

    async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        RoutedApiClient::stream(self, prompt).await
    }
}
