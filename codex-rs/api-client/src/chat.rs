use std::time::Duration;

use async_trait::async_trait;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::protocol::SessionSource;
use futures::TryStreamExt;
use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::client::PayloadBuilder;
use crate::common::backoff;
use crate::error::Error;
use crate::error::Result;
use crate::model_provider::ModelProviderInfo;
use crate::prompt::Prompt;
use crate::stream::ResponseEvent;
use crate::stream::ResponseStream;

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

#[async_trait]
impl ApiClient for ChatCompletionsApiClient {
    type Config = ChatCompletionsApiClientConfig;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Self { config })
    }

    async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        Self::validate_prompt(prompt)?;

        let payload = crate::payload::chat::ChatPayloadBuilder::new(self.config.model.clone())
            .build(prompt)?;
        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

        let mut attempt: i64 = 0;
        let max_retries = self.config.provider.request_max_retries();

        loop {
            attempt += 1;

            let req_builder = crate::client::http::build_request(
                &self.config.http_client,
                &self.config.provider,
                &None,
                Some(&self.config.session_source),
                &[],
            )
            .await?;

            let res = self
                .config
                .otel_event_manager
                .log_request(attempt as u64, || {
                    req_builder
                        .header(reqwest::header::ACCEPT, "text/event-stream")
                        .json(&payload)
                        .send()
                })
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    let stream = resp
                        .bytes_stream()
                        .map_err(|err| Error::ResponseStreamFailed {
                            source: err,
                            request_id: None,
                        });
                    let idle_timeout = self.config.provider.stream_idle_timeout();
                    let otel = self.config.otel_event_manager.clone();
                    tokio::spawn(crate::client::sse::process_sse(
                        stream,
                        tx_event.clone(),
                        idle_timeout,
                        otel,
                        crate::decode::chat::ChatSseDecoder::new(),
                    ));

                    return Ok(ResponseStream { rx_event });
                }
                Ok(resp) => {
                    if attempt >= max_retries {
                        let status = resp.status();
                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response>".to_string());
                        return Err(Error::UnexpectedStatus { status, body });
                    }

                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<i64>().ok())
                        .map(|secs| Duration::from_secs(if secs < 0 { 0 } else { secs as u64 }));
                    tokio::time::sleep(retry_after.unwrap_or_else(|| backoff(attempt))).await;
                }
                Err(error) => {
                    if attempt >= max_retries {
                        return Err(Error::Http(error));
                    }
                    tokio::time::sleep(backoff(attempt)).await;
                }
            }
        }
    }
}

impl ChatCompletionsApiClient {
    fn validate_prompt(prompt: &Prompt) -> Result<()> {
        if prompt.output_schema.is_some() {
            return Err(Error::UnsupportedOperation(
                "output_schema is not supported for Chat Completions API".to_string(),
            ));
        }
        Ok(())
    }
}
