use std::collections::HashMap;

use codex_api::ApiError;
use codex_api::Reasoning;
use codex_api::ResponseEvent;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesWebsocketClient;
use codex_api::ResponsesWebsocketConnection;
use codex_api::ResponsesWsRequest;
use codex_api::build_session_headers;
use codex_api::create_text_param_for_request;
use codex_http_client::HttpClientFactory;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::CodexAuth;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::default_headers;
use codex_model_provider::AgentIdentitySessionFallback;
use codex_model_provider::ProviderAuthScope;
use codex_model_provider::SharedModelProvider;
use codex_protocol::error::CodexErr;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::SessionSource;
use http::HeaderValue;
use serde_json::Value;
use thiserror::Error;

const MODEL: &str = "gpt-5.6-luna";
const MAX_OUTPUT_BYTES: usize = 8 * 1024;
const RESPONSES_WEBSOCKETS_BETA: &str = "responses_websockets=2026-02-06";
const RESPONSES_LITE_METADATA_KEY: &str =
    "ws_request_header_x_openai_internal_codex_responses_lite";

/// Host-owned provider, authentication, and attribution for one Luna connection.
pub struct LunaSamplerConfig {
    /// Provider and credentials selected for the owning thread.
    pub provider: SharedModelProvider,
    /// Effective proxy, custom-CA, and cookie configuration.
    pub http_client_factory: HttpClientFactory,
    /// Agent-identity policy selected for the owning thread.
    pub agent_identity_policy: AgentIdentityAuthPolicy,
    /// Host-resolved source used to scope agent-identity authentication.
    pub session_source: SessionSource,
    /// Owning runtime session identifier.
    pub session_id: String,
    /// Owning thread identifier.
    pub thread_id: String,
    /// Optional host-resolved request originator.
    pub originator: Option<String>,
    /// Optional inference service tier.
    pub service_tier: Option<String>,
}

/// One tool-less structured Luna request over an already-open connection.
pub struct LunaSamplingRequest {
    /// Trusted instructions describing the requested classification.
    pub instructions: String,
    /// Untrusted input that the model should classify.
    pub input: String,
    /// Strict JSON schema constraining the model response.
    pub output_schema: Value,
    /// Reasoning budget explicitly selected for this request.
    pub reasoning_effort: ReasoningEffort,
    /// Owning turn identifier used for request attribution.
    pub turn_id: String,
}

/// Failures returned while connecting or sampling the Luna model.
#[derive(Debug, Error)]
pub enum LunaSamplerError {
    /// The thread's provider or scoped credentials could not be resolved.
    #[error("could not resolve the Luna model provider: {0}")]
    Provider(#[source] CodexErr),
    /// The Responses WebSocket could not be opened or streamed.
    #[error("Luna Responses WebSocket failed: {0}")]
    Api(#[source] ApiError),
    /// The provider's WebSocket connect deadline elapsed.
    #[error("Luna Responses WebSocket connection timed out")]
    ConnectionTimeout,
    /// The response did not contain an assistant text value.
    #[error("Luna response did not contain assistant output")]
    MissingOutput,
    /// The response exceeded the bounded output limit.
    #[error("Luna response exceeded the output limit")]
    OutputTooLarge,
}

/// A persistent, authenticated Responses WebSocket dedicated to Luna sampling.
pub struct LunaSampler {
    connection: ResponsesWebsocketConnection,
    session_id: String,
    thread_id: String,
    service_tier: Option<String>,
}

impl LunaSampler {
    /// Opens the WebSocket before any sample is requested.
    pub async fn connect(config: LunaSamplerConfig) -> Result<Self, LunaSamplerError> {
        let provider = config
            .provider
            .api_provider()
            .await
            .map_err(LunaSamplerError::Provider)?;
        let auth = config
            .provider
            .api_auth_for_scope(ProviderAuthScope {
                agent_identity_policy: config.agent_identity_policy,
                session_source: config.session_source,
                agent_identity_session_fallback: AgentIdentitySessionFallback::default(),
            })
            .await
            .map_err(LunaSamplerError::Provider)?
            .auth;
        let mut headers = build_session_headers(
            Some(config.session_id.clone()),
            Some(config.thread_id.clone()),
        );
        headers.insert(
            "openai-beta",
            HeaderValue::from_static(RESPONSES_WEBSOCKETS_BETA),
        );
        if let Some(originator) = config.originator.as_deref() {
            add_originator_header(&mut headers, originator);
        }
        if let Ok(request_id) = HeaderValue::from_str(&config.thread_id) {
            headers.insert("x-client-request-id", request_id);
        }

        let provider_info = config.provider.info();
        if config
            .provider
            .auth()
            .await
            .as_ref()
            .is_some_and(CodexAuth::uses_codex_backend)
            && provider_info.is_openai()
            && provider_info.requires_openai_auth
            && provider_info.env_key.is_none()
            && provider_info.experimental_bearer_token.is_none()
            && provider_info.auth.is_none()
            && provider_info.aws.is_none()
        {
            let routing_hint = match config.service_tier.as_deref() {
                Some(tier) => format!("model={MODEL};tier={tier}"),
                None => format!("model={MODEL}"),
            };
            if let Ok(value) = HeaderValue::from_str(&routing_hint) {
                headers.insert("x-codex-routing-hint", value);
            }
        }

        let client = ResponsesWebsocketClient::new(provider, auth);
        let connect = client.connect(
            &config.http_client_factory,
            headers,
            default_headers(),
            /*turn_state*/ None,
            /*telemetry*/ None,
        );
        let connection = tokio::time::timeout(provider_info.websocket_connect_timeout(), connect)
            .await
            .map_err(|_| LunaSamplerError::ConnectionTimeout)?
            .map_err(LunaSamplerError::Api)?;

        Ok(Self {
            connection,
            session_id: config.session_id,
            thread_id: config.thread_id,
            service_tier: config.service_tier,
        })
    }

    /// Sends one structured, tool-less request on the existing WebSocket.
    pub async fn sample(&self, request: LunaSamplingRequest) -> Result<String, LunaSamplerError> {
        let metadata = HashMap::from([
            ("session_id".to_owned(), self.session_id.clone()),
            ("thread_id".to_owned(), self.thread_id.clone()),
            ("turn_id".to_owned(), request.turn_id),
            (RESPONSES_LITE_METADATA_KEY.to_owned(), "true".to_owned()),
        ]);
        let request = ResponsesApiRequest {
            model: MODEL.to_owned(),
            instructions: String::new(),
            input: vec![
                ResponseItem::AdditionalTools {
                    id: None,
                    role: "developer".to_owned(),
                    tools: Vec::new(),
                },
                ResponseItem::Message {
                    id: None,
                    role: "developer".to_owned(),
                    content: vec![ContentItem::InputText {
                        text: request.instructions,
                    }],
                    phase: None,
                    internal_chat_message_metadata_passthrough: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "user".to_owned(),
                    content: vec![ContentItem::InputText {
                        text: request.input,
                    }],
                    phase: None,
                    internal_chat_message_metadata_passthrough: None,
                },
            ],
            tools: None,
            tool_choice: "none".to_owned(),
            parallel_tool_calls: false,
            reasoning: Some(Reasoning {
                effort: Some(request.reasoning_effort),
                summary: None,
                context: None,
            }),
            store: false,
            stream: true,
            stream_options: None,
            include: Vec::new(),
            service_tier: self.service_tier.clone(),
            prompt_cache_key: Some(format!("guardian-v2:{}", self.thread_id)),
            text: create_text_param_for_request(
                /*verbosity*/ None,
                &Some(request.output_schema),
                /*output_schema_strict*/ true,
            ),
            client_metadata: Some(metadata),
        };
        let mut stream = self
            .connection
            .stream_request(
                ResponsesWsRequest::ResponseCreate((&request).into()),
                /*connection_reused*/ true,
                /*turn_state*/ None,
            )
            .await
            .map_err(LunaSamplerError::Api)?;

        let mut output = String::new();
        let mut deltas = String::new();
        while let Some(event) = stream.rx_event.recv().await {
            match event.map_err(LunaSamplerError::Api)? {
                ResponseEvent::OutputTextDelta(delta) => {
                    deltas.push_str(&delta);
                    if deltas.len() > MAX_OUTPUT_BYTES {
                        return Err(LunaSamplerError::OutputTooLarge);
                    }

                    if serde_json::from_str::<serde_json::Map<String, Value>>(&deltas).is_ok() {
                        let mut remaining_events = stream.rx_event;
                        tokio::spawn(async move {
                            // The transport needs a live consumer until completion to keep its
                            // authenticated WebSocket available for the next sample.
                            while let Some(event) = remaining_events.recv().await {
                                if matches!(event, Ok(ResponseEvent::Completed { .. }) | Err(_)) {
                                    break;
                                }
                            }
                        });
                        return Ok(deltas);
                    }
                }
                ResponseEvent::OutputItemDone(ResponseItem::Message { role, content, .. })
                    if role == "assistant" =>
                {
                    for item in content {
                        if let ContentItem::OutputText { text } = item {
                            output.push_str(&text);
                        }
                    }
                }
                ResponseEvent::Completed { .. } => {
                    if !output.is_empty() {
                        return Ok(output);
                    }
                    if !deltas.is_empty() {
                        return Ok(deltas);
                    }
                    return Err(LunaSamplerError::MissingOutput);
                }
                _ => {}
            }
            if output.len() > MAX_OUTPUT_BYTES || deltas.len() > MAX_OUTPUT_BYTES {
                return Err(LunaSamplerError::OutputTooLarge);
            }
        }

        Err(LunaSamplerError::MissingOutput)
    }
}

#[cfg(test)]
#[path = "sampler_tests.rs"]
mod tests;
