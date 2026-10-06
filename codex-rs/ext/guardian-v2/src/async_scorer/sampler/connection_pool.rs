//! Keeps WebSocket establishment off the classification path.
//! One background worker fills the pool with bounded concurrent opens before it empties.
//! Opening timeouts pause replenishment without discarding healthy in-flight opens;
//! requests with no healthy idle socket use HTTP under the same concurrency limit.
//! Each lease resolves workspace routing before reusing a socket or starting HTTP.
//! Constrained HTTP clients reject redirects, including after a cache policy change.
//! TLS roots are loaded once per sampler, like the pooled HTTP transport configuration.

use super::super::metrics::sampler_failure_reason;
use super::INITIAL_WEBSOCKET_CONNECTIONS;
use super::LunaSamplerConfig;
use super::LunaSamplerError;
use super::MAX_CONCURRENT_REQUESTS;
use codex_api::ApiError;
use codex_api::Provider;
use codex_api::ReqwestTransport;
use codex_api::ResponseStream;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesClient;
use codex_api::ResponsesOptions;
use codex_api::ResponsesWebsocketClient;
use codex_api::ResponsesWebsocketConnection;
use codex_api::ResponsesWsRequest;
use codex_api::SharedAuthProvider;
use codex_api::TransportError;
use codex_api::build_session_headers;
use codex_http_client::ClientRouteClass;
use codex_login::CodexAuth;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::create_client_for_route_async;
use codex_login::default_client::default_headers;
use codex_model_provider::ACCOUNT_ROUTING_HEADER;
use codex_model_provider::AgentIdentitySessionFallback;
use codex_model_provider::ProviderAuthScope;
use codex_model_provider::ResolvedResponsesProvider;
use codex_model_provider::ResponsesConnectionKey;
use codex_protocol::ThreadId;
use codex_websocket_client::WebSocketConnector;
use http::HeaderMap;
use http::HeaderValue;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::sync::OwnedMutexGuard;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tokio::time::Instant;

const CONNECT_COOLDOWN: Duration = Duration::from_secs(5 * 60);
const MAX_WEBSOCKET_AGE: Duration = Duration::from_secs(55 * 60);
const MAX_CONCURRENT_OPENS: usize = 4;
const REFILL_LOW_WATERMARK: usize = INITIAL_WEBSOCKET_CONNECTIONS / 2;
const RESPONSES_WEBSOCKETS_BETA: &str = "responses_websockets=2026-02-06";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestMode {
    Regular,
    GuardianClassifier,
}

pub(super) struct ConnectionPool {
    config: Arc<LunaSamplerConfig>,
    pub(super) idle_connections: Mutex<Vec<PooledConnection>>,
    classifications: Arc<Semaphore>,
    // A socket holds its permit while opening, idle, leased, and draining.
    sockets: Arc<Semaphore>,
    replenishing: Arc<tokio::sync::Mutex<()>>,
    retry_after: Mutex<Option<Instant>>,
    http_transport: Mutex<Option<Arc<HttpTransport>>>,
    websocket_connector: OnceCell<WebSocketConnector>,
}

struct HttpTransport {
    url: String,
    redirect_policy: ClientRedirectPolicy,
    transport: OnceCell<ReqwestTransport>,
}

pub(super) struct PooledConnection {
    connection: ResponsesWebsocketConnection,
    request_kind: RequestMode,
    // The bridge routes by thread ID, so each socket needs its own identity.
    pub(super) thread_id: String,
    pub(super) expires_at: Instant,
    auth_changes: Option<tokio::sync::watch::Receiver<u64>>,
    key: ResponsesConnectionKey,
    _permit: OwnedSemaphorePermit,
}

struct ClientSetup {
    provider: Provider,
    auth: SharedAuthProvider,
    redirect_policy: ClientRedirectPolicy,
    key: ResponsesConnectionKey,
    auth_changes: Option<tokio::sync::watch::Receiver<u64>>,
}

enum Connection {
    Websocket(PooledConnection),
    Http(ResponsesClient<ReqwestTransport>),
}

pub(super) struct ConnectionLease {
    pub(super) thread_id: String,
    pub(super) request_kind: RequestMode,
    connection: Connection,
    pool: Arc<ConnectionPool>,
    _permit: OwnedSemaphorePermit,
}

impl ConnectionPool {
    pub(super) fn new(config: Arc<LunaSamplerConfig>) -> Arc<Self> {
        Arc::new(Self {
            config,
            idle_connections: Mutex::new(Vec::new()),
            classifications: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            sockets: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            replenishing: Arc::new(tokio::sync::Mutex::new(())),
            retry_after: Mutex::new(None),
            http_transport: Mutex::new(None),
            websocket_connector: OnceCell::new(),
        })
    }

    pub(super) fn clear(&self) {
        self.idle_connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// Never waits for another opener or sleeps through the cooldown.
    pub(super) fn replenish(self: &Arc<Self>) -> Option<JoinHandle<()>> {
        let guard = Arc::clone(&self.replenishing).try_lock_owned().ok()?;
        self.start_refill(guard)
    }

    pub(super) async fn prewarm(self: &Arc<Self>) {
        // Explicit prewarming waits for any existing refill before checking readiness.
        let guard = Arc::clone(&self.replenishing).lock_owned().await;
        if let Some(refill) = self.start_refill(guard) {
            let _ = refill.await;
        }
    }

    fn start_refill(self: &Arc<Self>, guard: OwnedMutexGuard<()>) -> Option<JoinHandle<()>> {
        let mut retry_after = self
            .retry_after
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if retry_after.is_some_and(|deadline| Instant::now() < deadline) {
            return None;
        }
        *retry_after = None;
        drop(retry_after);
        let pool = Arc::clone(self);
        Some(tokio::spawn(async move {
            let _guard = guard;
            let mut opening = JoinSet::new();
            let mut failed = false;
            loop {
                let idle = pool
                    .idle_connections
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .len();
                while !failed
                    && idle + opening.len() < INITIAL_WEBSOCKET_CONNECTIONS
                    && opening.len() < MAX_CONCURRENT_OPENS
                {
                    let Ok(permit) = Arc::clone(&pool.sockets).try_acquire_owned() else {
                        break;
                    };
                    let pool = Arc::clone(&pool);
                    opening.spawn(async move { pool.open_connection(permit).await });
                }
                let Some(result) = opening.join_next().await else {
                    break;
                };
                match result {
                    Ok(Ok(connection)) => {
                        let mut idle = pool
                            .idle_connections
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        // Returned leases may have refilled the pool while we opened.
                        if idle.len() < INITIAL_WEBSOCKET_CONNECTIONS {
                            idle.push(connection);
                        }
                    }
                    Ok(Err(error)) => {
                        failed = true;
                        if matches!(error, LunaSamplerError::ConnectionTimeout) {
                            *pool
                                .retry_after
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                                Some(Instant::now() + CONNECT_COOLDOWN);
                        }
                    }
                    Err(_) => failed = true,
                }
            }
        }))
    }

    pub(super) async fn lease(self: &Arc<Self>) -> Result<ConnectionLease, LunaSamplerError> {
        let started = Instant::now();
        let permit = Arc::clone(&self.classifications)
            .acquire_owned()
            .await
            .map_err(|error| LunaSamplerError::Api(ApiError::Stream(error.to_string())))?;
        let admitted = Instant::now();
        let ClientSetup {
            mut provider,
            auth,
            redirect_policy,
            key,
            auth_changes,
        } = self.client_setup().await?;
        let setup_finished = Instant::now();
        let connection = loop {
            let idle = self
                .idle_connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop();
            match idle {
                Some(connection)
                    if connection.key == key
                        && connection
                            .auth_changes
                            .as_ref()
                            .is_none_or(|auth| !auth.has_changed().unwrap_or(true))
                        && Instant::now() < connection.expires_at
                        && !connection.connection.is_closed().await =>
                {
                    break Some(connection);
                }
                Some(_) => {}
                None => break None,
            }
        };
        let (connection, thread_id, request_kind) = match connection {
            Some(connection) => {
                if self
                    .idle_connections
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .len()
                    <= REFILL_LOW_WATERMARK
                {
                    self.replenish();
                }
                let thread_id = connection.thread_id.clone();
                let request_kind = connection.request_kind;
                (Connection::Websocket(connection), thread_id, request_kind)
            }
            None => {
                self.replenish();
                // Sampling owns the retry budget across both transports.
                provider.retry.max_attempts = 0;
                let request_kind = self.responses_request_kind().await;
                let url = provider.url_for_path("/responses");
                let redirect_policy = if provider.headers.contains_key(ACCOUNT_ROUTING_HEADER) {
                    ClientRedirectPolicy::Reject
                } else {
                    redirect_policy
                };
                let transport = {
                    let mut cached = self
                        .http_transport
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(transport) = cached.as_ref()
                        && transport.url == url
                        && transport.redirect_policy == redirect_policy
                    {
                        Arc::clone(transport)
                    } else {
                        let transport = Arc::new(HttpTransport {
                            url: url.clone(),
                            redirect_policy,
                            transport: OnceCell::new(),
                        });
                        *cached = Some(Arc::clone(&transport));
                        transport
                    }
                };
                let transport = transport
                    .transport
                    .get_or_try_init(|| async {
                        let client = create_client_for_route_async(
                            self.config.http_client_factory.clone(),
                            url,
                            ClientRouteClass::Api,
                            redirect_policy,
                        )
                        .await
                        .map_err(|error| {
                            LunaSamplerError::Api(ApiError::Transport(TransportError::Build(
                                error.to_string(),
                            )))
                        })?;
                        Ok::<_, LunaSamplerError>(ReqwestTransport::from_http_client(client))
                    })
                    .await?
                    .clone();
                let client = ResponsesClient::new(transport, provider, auth);
                (
                    Connection::Http(client),
                    ThreadId::new().to_string(),
                    request_kind,
                )
            }
        };
        if auth_changes
            .as_ref()
            .is_some_and(|auth| auth.has_changed().unwrap_or(true))
        {
            return Err(LunaSamplerError::Api(ApiError::Stream(
                "authentication changed while leasing".into(),
            )));
        }
        if let Some(metrics) = self.config.metrics.as_deref() {
            let transport = match &connection {
                Connection::Websocket(_) => "websocket",
                Connection::Http(_) => "http",
            };
            for (stage, duration) in [
                ("admission", admitted.duration_since(started)),
                ("setup", setup_finished.duration_since(admitted)),
                ("total", started.elapsed()),
            ] {
                metrics.histogram(
                    "codex.guardian_v2.connection.lease.duration_ms",
                    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
                    &[("stage", stage), ("transport", transport)],
                );
            }
        }
        Ok(ConnectionLease {
            thread_id,
            request_kind,
            connection,
            pool: Arc::clone(self),
            _permit: permit,
        })
    }

    async fn client_setup(&self) -> Result<ClientSetup, LunaSamplerError> {
        let mut auth_changes = self
            .config
            .provider
            .auth_manager()
            .map(|manager| manager.auth_change_receiver());
        let revision = auth_changes.as_mut().map(|auth| *auth.borrow_and_update());
        let ResolvedResponsesProvider {
            provider,
            redirect_policy,
        } = self
            .config
            .provider
            .responses_api_provider(&self.config.workspace_routing)
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
        if auth_changes
            .as_ref()
            .is_some_and(|auth| auth.has_changed().unwrap_or(true))
        {
            return Err(LunaSamplerError::Api(ApiError::Stream(
                "authentication changed while resolving routing".into(),
            )));
        }
        let key = ResponsesConnectionKey::new(&provider, revision);
        Ok(ClientSetup {
            provider,
            auth,
            redirect_policy,
            key,
            auth_changes,
        })
    }

    fn headers(
        &self,
        thread_id: &str,
        request_kind: RequestMode,
    ) -> Result<HeaderMap, LunaSamplerError> {
        let mut headers = build_session_headers(
            Some(self.config.session_id.clone()),
            Some(thread_id.to_owned()),
        );
        if request_kind == RequestMode::GuardianClassifier {
            headers.insert("x-codex-guardian", HeaderValue::from_static("classifier"));
        }
        headers.insert("x-openai-subagent", HeaderValue::from_static("guardian"));
        headers.insert(
            "x-codex-window-id",
            HeaderValue::from_str(&format!("{thread_id}:0")).map_err(|error| {
                LunaSamplerError::Api(ApiError::Stream(format!(
                    "invalid classifier window ID: {error}"
                )))
            })?,
        );
        headers.insert(
            "x-openai-internal-codex-responses-lite",
            HeaderValue::from_static("true"),
        );
        if let Some(originator) = self.config.originator.as_deref() {
            add_originator_header(&mut headers, originator);
        }
        if let Ok(request_id) = HeaderValue::from_str(thread_id) {
            headers.insert("x-client-request-id", request_id);
        }
        Ok(headers)
    }
    async fn responses_request_kind(&self) -> RequestMode {
        let provider = self.config.provider.info();
        if self
            .config
            .provider
            .auth()
            .await
            .as_ref()
            .is_some_and(CodexAuth::uses_codex_backend)
            && provider.supports_codex_backend_routes()
            && provider.requires_openai_auth
            && provider.env_key.is_none()
            && provider.experimental_bearer_token.is_none()
            && provider.auth.is_none()
            && provider.aws.is_none()
        {
            RequestMode::GuardianClassifier
        } else {
            RequestMode::Regular
        }
    }

    async fn open_connection(
        &self,
        permit: OwnedSemaphorePermit,
    ) -> Result<PooledConnection, LunaSamplerError> {
        let ClientSetup {
            provider,
            auth,
            redirect_policy: _,
            key,
            auth_changes,
        } = self.client_setup().await?;
        let thread_id = ThreadId::new().to_string();
        let request_kind = self.responses_request_kind().await;
        let mut headers = self.headers(&thread_id, request_kind)?;
        headers.insert(
            "openai-beta",
            HeaderValue::from_static(RESPONSES_WEBSOCKETS_BETA),
        );

        let provider_info = self.config.provider.info();
        let client = ResponsesWebsocketClient::new(provider, auth);
        let connect = async {
            let connector = self
                .websocket_connector
                .get_or_try_init(|| async {
                    let factory = self.config.http_client_factory.clone();
                    tokio::task::spawn_blocking(move || WebSocketConnector::new(&factory))
                        .await
                        .map_err(|error| {
                            ApiError::Stream(format!("websocket TLS setup task failed: {error}"))
                        })?
                        .map_err(|error| {
                            ApiError::Stream(format!("failed to configure websocket TLS: {error}"))
                        })
                })
                .await?;
            client
                .connect_with_connector(
                    connector,
                    headers,
                    default_headers(),
                    /*turn_state*/ None,
                    /*telemetry*/ None,
                )
                .await
        };
        let started_at = Instant::now();
        let result = tokio::time::timeout(provider_info.websocket_connect_timeout(), connect)
            .await
            .map_err(|_| LunaSamplerError::ConnectionTimeout)
            .and_then(|result| result.map_err(LunaSamplerError::Api));
        if let Some(metrics) = self.config.metrics.as_deref() {
            let outcome = if result.is_ok() { "success" } else { "failure" };
            let mut tags = vec![("endpoint", "/responses"), ("outcome", outcome)];
            if let Err(error) = &result {
                tags.push(("failure_reason", sampler_failure_reason(error)));
            }
            metrics.histogram(
                "codex.guardian_v2.connection.duration_ms",
                i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX),
                &tags,
            );
        }
        let connection = result?;
        if auth_changes
            .as_ref()
            .is_some_and(|auth| auth.has_changed().unwrap_or(true))
        {
            return Err(LunaSamplerError::Api(ApiError::Stream(
                "authentication changed while connecting".into(),
            )));
        }

        Ok(PooledConnection {
            connection,
            request_kind,
            thread_id,
            expires_at: Instant::now() + MAX_WEBSOCKET_AGE,
            auth_changes,
            key,
            _permit: permit,
        })
    }
}

impl ConnectionLease {
    pub(super) async fn stream_request(
        &self,
        request: &ResponsesApiRequest,
    ) -> Result<ResponseStream, ApiError> {
        match &self.connection {
            Connection::Websocket(connection) => {
                connection
                    .connection
                    .stream_request(
                        ResponsesWsRequest::ResponseCreate(request.into()),
                        /*connection_reused*/ true,
                        /*turn_state*/ None,
                    )
                    .await
            }
            Connection::Http(client) => {
                // The SSE idle timeout starts after headers arrive. Bound that wait too.
                tokio::time::timeout(
                    self.pool.config.provider.info().stream_idle_timeout(),
                    client.stream_request(
                        request.clone(),
                        ResponsesOptions {
                            session_id: Some(self.pool.config.session_id.clone()),
                            thread_id: Some(self.thread_id.clone()),
                            extra_headers: self
                                .pool
                                .headers(&self.thread_id, self.request_kind)
                                .map_err(|error| ApiError::Stream(error.to_string()))?,
                            ..Default::default()
                        },
                    ),
                )
                .await
                .map_err(|_| ApiError::Transport(TransportError::Timeout))?
            }
        }
    }

    pub(super) fn reuse(self) {
        if let Connection::Websocket(connection) = self.connection {
            self.pool
                .idle_connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(connection);
        }
    }
}

#[cfg(test)]
#[path = "connection_pool_tests.rs"]
mod tests;
