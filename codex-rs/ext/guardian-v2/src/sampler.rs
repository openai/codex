use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_api::ApiError;
use codex_api::Reasoning;
use codex_api::ReasoningContext;
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
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

const MODEL: &str = "gpt-5.6-luna";
const MAX_OUTPUT_BYTES: usize = 8 * 1024;
const INITIAL_WEBSOCKET_CONNECTIONS: usize = 2;
const MAX_WEBSOCKET_CONNECTIONS: usize = 8;
const MAX_WEBSOCKET_AGE: Duration = Duration::from_secs(55 * 60);
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

struct PooledConnection {
    connection: ResponsesWebsocketConnection,
    connected_at: Instant,
}

struct ConnectionLease {
    connection: PooledConnection,
    idle_connections: Arc<Mutex<Vec<PooledConnection>>>,
    _permit: OwnedSemaphorePermit,
}

impl ConnectionLease {
    fn reuse(self) {
        self.idle_connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.connection);
    }
}

/// A bounded pool of authenticated Responses WebSockets dedicated to Luna sampling.
pub struct LunaSampler {
    config: LunaSamplerConfig,
    idle_connections: Arc<Mutex<Vec<PooledConnection>>>,
    capacity: Arc<Semaphore>,
}

impl LunaSampler {
    /// Opens the initial WebSockets before any sample is requested.
    pub async fn connect(config: LunaSamplerConfig) -> Result<Self, LunaSamplerError> {
        let sampler = Self {
            config,
            idle_connections: Arc::new(Mutex::new(Vec::with_capacity(MAX_WEBSOCKET_CONNECTIONS))),
            capacity: Arc::new(Semaphore::new(MAX_WEBSOCKET_CONNECTIONS)),
        };
        for index in 0..INITIAL_WEBSOCKET_CONNECTIONS {
            let connection = match sampler.open_connection().await {
                Ok(connection) => connection,
                Err(error) if index == 0 => return Err(error),
                Err(_) => break,
            };
            sampler
                .idle_connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(connection);
        }
        Ok(sampler)
    }

    async fn open_connection(&self) -> Result<PooledConnection, LunaSamplerError> {
        let provider = self
            .config
            .provider
            .api_provider()
            .await
            .map_err(LunaSamplerError::Provider)?;
        let auth = self
            .config
            .provider
            .api_auth_for_scope(ProviderAuthScope {
                agent_identity_policy: self.config.agent_identity_policy,
                session_source: self.config.session_source.clone(),
                agent_identity_session_fallback: AgentIdentitySessionFallback::default(),
            })
            .await
            .map_err(LunaSamplerError::Provider)?
            .auth;
        let mut headers = build_session_headers(
            Some(self.config.session_id.clone()),
            Some(self.config.thread_id.clone()),
        );
        headers.insert(
            "openai-beta",
            HeaderValue::from_static(RESPONSES_WEBSOCKETS_BETA),
        );
        headers.insert(
            "x-openai-internal-codex-responses-lite",
            HeaderValue::from_static("true"),
        );
        if let Some(originator) = self.config.originator.as_deref() {
            add_originator_header(&mut headers, originator);
        }
        if let Ok(request_id) = HeaderValue::from_str(&self.config.thread_id) {
            headers.insert("x-client-request-id", request_id);
        }

        let provider_info = self.config.provider.info();
        if self
            .config
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
            let routing_hint = match self.config.service_tier.as_deref() {
                Some(tier) => format!("model={MODEL};tier={tier}"),
                None => format!("model={MODEL}"),
            };
            if let Ok(value) = HeaderValue::from_str(&routing_hint) {
                headers.insert("x-codex-routing-hint", value);
            }
        }

        let client = ResponsesWebsocketClient::new(provider, auth);
        let connect = client.connect(
            &self.config.http_client_factory,
            headers,
            default_headers(),
            /*turn_state*/ None,
            /*telemetry*/ None,
        );
        let connection = tokio::time::timeout(provider_info.websocket_connect_timeout(), connect)
            .await
            .map_err(|_| LunaSamplerError::ConnectionTimeout)?
            .map_err(LunaSamplerError::Api)?;

        Ok(PooledConnection {
            connection,
            connected_at: Instant::now(),
        })
    }

    async fn lease_connection(&self) -> Result<ConnectionLease, LunaSamplerError> {
        let permit = Arc::clone(&self.capacity)
            .acquire_owned()
            .await
            .map_err(|_| LunaSamplerError::ConnectionTimeout)?;
        let connection = loop {
            let idle = self
                .idle_connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop();
            match idle {
                Some(connection)
                    if connection.connected_at.elapsed() < MAX_WEBSOCKET_AGE
                        && !connection.connection.is_closed().await =>
                {
                    break connection;
                }
                Some(_) => {}
                None => break self.open_connection().await?,
            }
        };
        Ok(ConnectionLease {
            connection,
            idle_connections: Arc::clone(&self.idle_connections),
            _permit: permit,
        })
    }

    /// Sends one structured, tool-less request on an exclusively leased WebSocket.
    pub async fn sample(&self, request: LunaSamplingRequest) -> Result<String, LunaSamplerError> {
        let metadata = HashMap::from([
            ("session_id".to_owned(), self.config.session_id.clone()),
            ("thread_id".to_owned(), self.config.thread_id.clone()),
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
                context: Some(ReasoningContext::AllTurns),
            }),
            store: false,
            stream: true,
            stream_options: None,
            include: Vec::new(),
            service_tier: self.config.service_tier.clone(),
            prompt_cache_key: Some(format!("guardian-v2:{}", self.config.thread_id)),
            text: create_text_param_for_request(
                /*verbosity*/ None,
                &Some(request.output_schema),
                /*output_schema_strict*/ true,
            ),
            client_metadata: Some(metadata),
        };
        let mut retried = false;
        'retry: loop {
            let lease = self.lease_connection().await?;
            let mut stream = lease
                .connection
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
                let event = match event {
                    Ok(event) => event,
                    Err(error)
                        if !retried
                            && matches!(
                                error,
                                ApiError::Retryable { .. } | ApiError::Stream(_)
                            ) =>
                    {
                        retried = true;
                        continue 'retry;
                    }
                    Err(error) => return Err(LunaSamplerError::Api(error)),
                };
                match event {
                    ResponseEvent::OutputTextDelta(delta) => {
                        deltas.push_str(&delta);
                        if deltas.len() > MAX_OUTPUT_BYTES {
                            return Err(LunaSamplerError::OutputTooLarge);
                        }

                        if serde_json::from_str::<serde_json::Map<String, Value>>(&deltas).is_ok() {
                            let mut remaining_events = stream.rx_event;
                            tokio::spawn(async move {
                                while let Some(event) = remaining_events.recv().await {
                                    match event {
                                        Ok(ResponseEvent::Completed { .. }) => {
                                            lease.reuse();
                                            break;
                                        }
                                        Err(_) => break,
                                        _ => {}
                                    }
                                }
                            });
                            return Ok(deltas);
                        }
                    }
                    ResponseEvent::OutputItemDone(ResponseItem::Message {
                        role, content, ..
                    }) if role == "assistant" => {
                        for item in content {
                            if let ContentItem::OutputText { text } = item {
                                output.push_str(&text);
                            }
                        }
                    }
                    ResponseEvent::Completed { .. } => {
                        lease.reuse();
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
            return Err(LunaSamplerError::MissingOutput);
        }
    }
}

#[cfg(test)]
#[path = "sampler_tests.rs"]
mod tests;
