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
use crate::stream::WireTokenUsage;
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
    Ok(match ev {
        crate::stream::ResponseEvent::Created => WireEvent::Created,
        crate::stream::ResponseEvent::OutputItemDone(item) => {
            WireEvent::OutputItemDone(serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
        }
        crate::stream::ResponseEvent::OutputItemAdded(item) => WireEvent::OutputItemAdded(
            serde_json::to_value(item).unwrap_or(serde_json::Value::Null),
        ),
        crate::stream::ResponseEvent::Completed {
            response_id,
            token_usage,
        } => {
            let mapped = token_usage.map(|u| WireTokenUsage {
                input_tokens: u.input_tokens,
                cached_input_tokens: u.cached_input_tokens,
                output_tokens: u.output_tokens,
                reasoning_output_tokens: u.reasoning_output_tokens,
                total_tokens: u.total_tokens,
            });
            WireEvent::Completed {
                response_id,
                token_usage: mapped,
            }
        }
        crate::stream::ResponseEvent::OutputTextDelta(s) => WireEvent::OutputTextDelta(s),
        crate::stream::ResponseEvent::ReasoningSummaryDelta(s) => {
            WireEvent::ReasoningSummaryDelta(s)
        }
        crate::stream::ResponseEvent::ReasoningContentDelta(s) => {
            WireEvent::ReasoningContentDelta(s)
        }
        crate::stream::ResponseEvent::ReasoningSummaryPartAdded => {
            WireEvent::ReasoningSummaryPartAdded
        }
        crate::stream::ResponseEvent::RateLimits(s) => {
            let to_win = |w: Option<codex_protocol::protocol::RateLimitWindow>| -> Option<crate::stream::WireRateLimitWindow> {
                w.map(|w| crate::stream::WireRateLimitWindow {
                    used_percent: Some(w.used_percent),
                    window_minutes: w.window_minutes,
                    resets_at: w.resets_at,
                })
            };
            WireEvent::RateLimits(crate::stream::WireRateLimitSnapshot {
                primary: to_win(s.primary),
                secondary: to_win(s.secondary),
            })
        }
    })
}
