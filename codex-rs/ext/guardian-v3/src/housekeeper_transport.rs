//! Responses transport for one serial housekeeper. Resolve the account and route for
//! each attempt. The sampler can resend a batch after a transient failure, but only a
//! completed routing response can reach the board.
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::ensure;
use codex_api::ApiError;
use codex_api::ResponseInterrupt;
use codex_api::ResponseStream;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesClient;
use codex_api::ResponsesOptions;
use codex_api::ResponsesWebsocketClient;
use codex_api::ResponsesWebsocketConnection;
use codex_api::ResponsesWsRequest;
use codex_api::TransportError;
use codex_api::build_session_headers;
use codex_core::CodexThread;
use codex_core::config::Config;
use codex_features::Feature;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicy;
use codex_http_client::RetryAfter;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::AuthOwner;
use codex_login::UnauthorizedRecovery;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::default_headers;
use codex_model_provider::ProviderAuthContext;
use codex_model_provider::ProviderAuthScope;
use codex_model_provider::ResponsesConnectionKey;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::WorkspaceRoutingContext;
use codex_model_provider::resolve_request;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_websocket_client::WebSocketConnector;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use tokio::time::Instant;

// Managed 401 recovery has its own bounded sequence.
const MAX_SAMPLING_RETRIES: usize = 2;

/// One inference turn across connection attempts for the same routing batch.
pub(super) struct BatchAttempt {
    turn: String,
    transport_retries: usize,
    auth_recovery: Option<UnauthorizedRecovery>,
}

pub(super) struct Transport {
    provider: SharedModelProvider,
    auth: ProviderAuthScope,
    pub(super) owner: Option<AuthOwner>,
    factory: HttpClientFactory,
    routing: Arc<WorkspaceRoutingContext>,
    originator: String,
    connector: Option<WebSocketConnector>,
    // Avoid paying another failed handshake on every batch of an HTTP-only endpoint.
    retry_websocket: Option<(ResponsesConnectionKey, Instant)>,
    socket: Option<(
        ResponsesConnectionKey,
        NetworkPolicy,
        ResponsesWebsocketConnection,
    )>,
}

/// Interrupt an unfinished response when routing is cancelled or times out.
pub(super) struct PendingResponse {
    pub(super) stream: ResponseStream,
}

impl Drop for PendingResponse {
    fn drop(&mut self) {
        if let Some(interrupt) = self.stream.interrupt.take() {
            let _ = interrupt.send(ResponseInterrupt::Cancel {
                deadline: Instant::now() + Duration::from_secs(1),
            });
        }
    }
}

impl Transport {
    pub(super) fn new(
        parent: &CodexThread,
        config: &Config,
        originator: String,
    ) -> anyhow::Result<Self> {
        let data = parent.thread_extension_data();
        let auth = data
            .get::<ProviderAuthContext>()
            .context("missing provider authentication for Guardian routing")?;
        let provider = auth.model_provider(config.model_provider.clone());
        // A provider command issues opaque bearer tokens. Rotation is expected between batches.
        let owner = if provider.info().auth.is_some() {
            None
        } else {
            codex_model_provider::request_auth_owner(provider.as_ref())
        };
        Ok(Self {
            provider,
            owner,
            auth: ProviderAuthScope {
                session_source: SessionSource::Internal(InternalSessionSource::Guardian),
                agent_identity_policy: if config.features.enabled(Feature::UseAgentIdentity) {
                    AgentIdentityAuthPolicy::ChatGptAuth
                } else {
                    AgentIdentityAuthPolicy::JwtOnly
                },
                agent_identity_session_fallback: Default::default(),
            },
            routing: data.get_or_init(|| config.workspace_routing_context()),
            factory: config.http_client_factory(),
            originator,
            connector: None,
            socket: None,
            retry_websocket: None,
        })
    }

    pub(super) fn start_batch(&self) -> BatchAttempt {
        BatchAttempt {
            turn: uuid::Uuid::now_v7().to_string(),
            transport_retries: 0,
            auth_recovery: self
                .provider
                .auth_manager()
                .map(|manager| manager.unauthorized_recovery()),
        }
    }

    pub(super) async fn retry_after_failure(
        &mut self,
        attempt: &mut BatchAttempt,
        error: &anyhow::Error,
    ) -> bool {
        if self.owner.as_ref().is_some_and(|owner| !owner.is_current()) {
            return false;
        }
        let Some(error) = error.downcast_ref::<ApiError>() else {
            return false;
        };
        let transport_retry = match error {
            ApiError::Api { status, .. }
            | ApiError::Transport(TransportError::Http { status, .. }) => {
                if *status == StatusCode::UNAUTHORIZED {
                    let Some(recovery) = &mut attempt.auth_recovery else {
                        return false;
                    };
                    if !recovery.has_next() || recovery.next().await.is_err() {
                        return false;
                    }
                    false
                } else if status.is_server_error() || *status == StatusCode::TOO_MANY_REQUESTS {
                    true
                } else {
                    return false;
                }
            }
            ApiError::Retryable { .. }
            | ApiError::RateLimitExceeded { .. }
            | ApiError::Stream(_)
            | ApiError::WebsocketMessageTooLarge
            | ApiError::ContentFilter
            | ApiError::ServerOverloaded { .. }
            | ApiError::PolicyCapacityUnavailable { .. }
            | ApiError::FlexUnavailable
            | ApiError::Transport(
                TransportError::RetryLimit
                | TransportError::Timeout
                | TransportError::Connection(_)
                | TransportError::Network(_),
            ) => true,
            _ => return false,
        };
        if transport_retry {
            if attempt.transport_retries == MAX_SAMPLING_RETRIES {
                return false;
            }
            attempt.transport_retries += 1;
        }
        self.socket = None;
        if let Some(advice) = retry_after(error) {
            tokio::time::sleep_until(advice.deadline()).await;
        }
        true
    }

    pub(super) async fn stream(
        &mut self,
        parent: &CodexThread,
        conversation: ThreadId,
        attempt: &BatchAttempt,
        mut request: ResponsesApiRequest,
        responses_lite: bool,
    ) -> anyhow::Result<PendingResponse> {
        let mut setup = resolve_request(
            self.provider.as_ref(),
            Some(self.auth.clone()),
            Some(&self.routing),
            self.factory.clone(),
        )
        .await?;
        let auth_changes = setup.auth_owner.as_ref().map(AuthOwner::changes);
        let command_auth = self.provider.info().auth.is_some();
        let revision = if command_auth && setup.runtime_id.is_none() {
            // The command can return a new token during resolution. Key the socket to the
            // credential for this request, rather than the generation before the command ran.
            let state = setup
                .auth
                .as_ref()
                .and_then(|auth| self.provider.auth_manager()?.auth_change_state_for(auth))
                .ok_or_else(|| {
                    ApiError::Stream("provider credential changed during Guardian routing".into())
                })?;
            Some(state.generation)
        } else {
            setup.auth_revision
        };
        let resolved_owner = setup.auth_owner.clone();
        let check_owner = || {
            ensure!(
                self.owner.as_ref().is_none_or(AuthOwner::is_current)
                    && (command_auth
                        || resolved_owner
                            .as_ref()
                            .is_none_or(|owner| self.owner.as_ref() == Some(owner))),
                "account changed during Guardian routing"
            );
            if auth_changes
                .as_ref()
                .map(|changes| changes.borrow().generation)
                != revision
            {
                return Err(
                    ApiError::Stream("credentials changed during Guardian routing".into()).into(),
                );
            }
            Ok::<(), anyhow::Error>(())
        };
        check_owner()?;
        let ids = parent.startup_metadata();
        let turn = &attempt.turn;
        let turn_metadata = serde_json::json!({
            "session_id": ids.session_id, "thread_id": conversation, "parent_thread_id": ids.thread_id,
            "turn_id": turn, "root_turn_id": turn, "request_kind":"turn","turn_trigger":"guardian_housekeeper",
        }).to_string();
        let add_guardian_headers = |headers: &mut HeaderMap| -> anyhow::Result<()> {
            headers.insert("x-openai-subagent", HeaderValue::from_static("guardian"));
            headers.insert(
                "x-codex-parent-thread-id",
                ids.thread_id.to_string().parse()?,
            );
            headers.insert("x-codex-window-id", format!("{conversation}:0").parse()?);
            add_originator_header(headers, &self.originator);
            if responses_lite {
                headers.insert(
                    "x-openai-internal-codex-responses-lite",
                    HeaderValue::from_static("true"),
                );
            }
            Ok(())
        };
        let mut metadata = std::collections::HashMap::from([
            ("session_id".to_owned(), ids.session_id.to_string()),
            ("thread_id".to_owned(), conversation.to_string()),
            ("turn_id".to_owned(), turn.to_owned()),
            ("root_turn_id".to_owned(), turn.to_owned()),
            (
                "x-codex-parent-thread-id".to_owned(),
                ids.thread_id.to_string(),
            ),
            ("x-openai-subagent".to_owned(), "guardian".to_owned()),
            ("x-codex-window-id".to_owned(), format!("{conversation}:0")),
            ("x-codex-turn-metadata".to_owned(), turn_metadata.clone()),
        ]);
        if responses_lite {
            metadata.insert(
                "ws_request_header_x_openai_internal_codex_responses_lite".to_owned(),
                "true".to_owned(),
            );
        }
        request.client_metadata = Some(metadata);
        let key = ResponsesConnectionKey::new(&setup.api_provider, revision);
        let policy = setup.http_client_factory.network_policy();
        let mut reused = false;
        if let Some((old_key, old_policy, socket)) = &self.socket {
            reused = setup.runtime_id.is_none()
                && old_key == &key
                && old_policy == policy
                && !socket.is_closed().await;
            if !reused {
                self.socket = None;
            }
        }
        let cooling_down = self
            .retry_websocket
            .as_ref()
            .is_some_and(|(failed, until)| failed == &key && Instant::now() < *until);
        if !reused
            && attempt.transport_retries == 0
            && !cooling_down
            && self.provider.info().supports_websockets
        {
            if self.connector.is_none() {
                let factory = self.factory.clone();
                self.connector = Some(
                    tokio::task::spawn_blocking(move || WebSocketConnector::new(&factory))
                        .await??,
                );
            }
            let connector = self
                .connector
                .as_ref()
                .and_then(|connector| connector.with_account_binding(policy))
                .context("housekeeper WebSocket account policy mismatch")?;
            let mut upgrade = build_session_headers(
                Some(ids.session_id.to_string()),
                Some(conversation.to_string()),
            );
            add_guardian_headers(&mut upgrade)?;
            upgrade.insert("x-client-request-id", conversation.to_string().parse()?);
            upgrade.insert(
                "openai-beta",
                HeaderValue::from_static("responses_websockets=2026-02-06"),
            );
            let ws =
                ResponsesWebsocketClient::new(setup.api_provider.clone(), setup.api_auth.clone());
            check_owner()?;
            match tokio::time::timeout(
                self.provider.info().websocket_connect_timeout(),
                ws.connect_with_connector(
                    &connector,
                    upgrade,
                    default_headers(),
                    /*turn_state*/ None,
                    /*telemetry*/ None,
                ),
            )
            .await
            {
                Ok(Ok(socket)) => self.socket = Some((key, policy.clone(), socket)),
                Ok(Err(
                    error @ ApiError::Transport(
                        TransportError::Policy(_) | TransportError::Build(_),
                    ),
                )) => return Err(error.into()),
                Ok(Err(error @ ApiError::Transport(TransportError::Http { status, .. })))
                | Ok(Err(error @ ApiError::Api { status, .. }))
                    if matches!(
                        status,
                        StatusCode::UNAUTHORIZED
                            | StatusCode::FORBIDDEN
                            | StatusCode::TOO_MANY_REQUESTS
                    ) || (status.is_server_error() && retry_after(&error).is_some()) =>
                {
                    return Err(error.into());
                }
                // As in GV2, give an unavailable upgrade five minutes before trying it again.
                _ => {
                    self.retry_websocket = Some((key, Instant::now() + Duration::from_secs(5 * 60)))
                }
            }
        }
        check_owner()?;
        let stream = if let Some((_, _, socket)) = &self.socket {
            socket
                .stream_request(
                    ResponsesWsRequest::ResponseCreate((&request).into()),
                    reused,
                    /*turn_state*/ None,
                )
                .await
        } else {
            let mut headers = HeaderMap::new();
            add_guardian_headers(&mut headers)?;
            headers.insert("x-codex-turn-metadata", turn_metadata.parse()?);
            // The sampler owns retries across both transports, including stream failures.
            setup.api_provider.retry.max_attempts = 0;
            let http = tokio::task::spawn_blocking(move || {
                let transport = setup.http_transport("/responses")?;
                Ok::<_, anyhow::Error>(ResponsesClient::new(
                    transport,
                    setup.api_provider,
                    setup.api_auth,
                ))
            })
            .await??;
            check_owner()?;
            http.stream_request(
                request,
                ResponsesOptions {
                    session_id: Some(ids.session_id.to_string()),
                    thread_id: Some(conversation.to_string()),
                    extra_headers: headers,
                    ..Default::default()
                },
            )
            .await
        }?;
        Ok(PendingResponse { stream })
    }
}

fn retry_after(error: &ApiError) -> Option<RetryAfter> {
    match error {
        ApiError::Transport(error) => error.retry_after(),
        ApiError::Retryable { retry_after, .. }
        | ApiError::RateLimitExceeded { retry_after, .. }
        | ApiError::ServerOverloaded { retry_after }
        | ApiError::PolicyCapacityUnavailable { retry_after } => *retry_after,
        _ => None,
    }
}
