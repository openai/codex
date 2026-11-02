//! Routes requests during HTTP fallback and publishes at most one background handshake.

use super::HttpFallbackPolicy;
use super::ModelClient;
use super::ModelClientSession;
use super::ResponseEvent;
use super::ResponsesConnectionKey;
use super::WebsocketConnectParams;
use super::WebsocketHandshake;
use super::WebsocketPreconnectMode;
use super::WebsocketRequestMode;
use super::WebsocketSession;
use crate::responses_metadata::CodexResponsesMetadata;
use crate::responses_retry::ResponsesRequestTransport;
use codex_http_client::RetryAfter;
use codex_otel::SessionTelemetry;
use codex_otel::auth_storage::AuthStorageOriginator;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::openai_models::ModelInfo;
use futures::FutureExt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::sync::Weak;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tokio_util::task::AbortOnDropHandle;

const RETRY_INTERVAL: Duration = Duration::from_secs(30);
pub(super) const RECOVERY_ACK_TIMEOUT: Duration = Duration::from_secs(5);

/// Foreground clients own probe cancellation. A handshake can only publish to a live owner.
#[derive(Debug, Clone)]
pub(super) enum WebsocketRoutes {
    Session(Arc<RouteOwner>),
    Probe,
}

#[derive(Debug, Default)]
pub(super) struct RouteOwner(Mutex<WebsocketTransport>);

impl RouteOwner {
    fn lock(&self) -> MutexGuard<'_, WebsocketTransport> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for WebsocketRoutes {
    fn default() -> Self {
        Self::Session(Arc::default())
    }
}

impl WebsocketRoutes {
    fn owner(&self) -> Option<&Arc<RouteOwner>> {
        match self {
            Self::Session(owner) => Some(owner),
            Self::Probe => None,
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.owner()
            .is_some_and(|owner| matches!(owner.lock().route, Route::Websocket))
    }

    pub(super) fn can_stream(&self) -> bool {
        self.owner()
            .is_some_and(|owner| matches!(owner.lock().route, Route::Websocket | Route::Ready(_)))
    }

    pub(super) fn fall_back(&self, policy: HttpFallbackPolicy) -> bool {
        let Some(owner) = self.owner() else {
            return false;
        };
        let mut transport = owner.lock();
        let activated = matches!(transport.route, Route::Websocket);
        match policy {
            HttpFallbackPolicy::StayOnHttp => transport.route = Route::HttpOnly,
            HttpFallbackPolicy::AllowProbes if activated => transport.route = Route::HttpFallback,
            HttpFallbackPolicy::AllowProbes => {}
        }
        activated
    }
}

#[derive(Debug, Default)]
struct WebsocketTransport {
    route: Route,
    // Even a proxy that accepts upgrades can consistently reject request frames.
    next_probe: Option<Instant>,
}

#[derive(Debug, Default)]
enum Route {
    #[default]
    Websocket,
    HttpOnly,
    HttpFallback,
    Connecting(AbortOnDropHandle<()>),
    Ready(Box<WebsocketSession>),
    Verifying,
}

pub(super) enum Connection {
    Websocket,
    Recovered(RecoveryAttempt),
}

/// Owns the provisional route until its first real request completes or is dropped.
pub(super) struct RecoveryAttempt {
    owner: Weak<RouteOwner>,
    retry_over_http: Option<oneshot::Sender<()>>,
}

impl RecoveryAttempt {
    pub(super) fn observe(&mut self, event: &ResponseEvent) {
        if matches!(
            event,
            ResponseEvent::GuardianAssessment(_)
                | ResponseEvent::OutputItemAdded(_)
                | ResponseEvent::OutputItemDone(_)
                | ResponseEvent::OutputTextDelta(_)
                | ResponseEvent::ToolCallInputDelta { .. }
                | ResponseEvent::ReasoningSummaryDelta { .. }
                | ResponseEvent::ReasoningSummaryDone { .. }
                | ResponseEvent::ReasoningContentDelta { .. }
                | ResponseEvent::ReasoningSummaryPartAdded { .. }
        ) {
            self.retry_over_http = None;
        }
    }

    pub(super) fn failed(mut self, error: &CodexErr) {
        let route = match error.details() {
            CodexErrorDetails::RequestTimeout => Route::HttpOnly,
            _ => Route::HttpFallback,
        };
        self.finish(route, error.retry_after());
        if let Some(retry) = self.retry_over_http.take() {
            let _ = retry.send(());
        }
    }

    pub(super) fn completed(mut self, telemetry: &SessionTelemetry) {
        if self.finish(Route::Websocket, /*retry_after*/ None) {
            telemetry.counter("codex.transport.recovered_websocket", /*inc*/ 1, &[]);
            tracing::info!("resumed WebSocket after HTTP fallback");
        }
    }

    fn finish(&mut self, route: Route, retry_after: Option<RetryAfter>) -> bool {
        // Consume the owner reference so Drop only acts on an abandoned attempt.
        let Some(owner) = std::mem::take(&mut self.owner).upgrade() else {
            return false;
        };
        let mut transport = owner.lock();
        if !matches!(transport.route, Route::Verifying) {
            return false;
        }
        if matches!(route, Route::HttpFallback) {
            transport.next_probe =
                Some(Instant::now() + RETRY_INTERVAL).max(retry_after.map(RetryAfter::deadline));
        }
        transport.route = route;
        true
    }
}

impl Drop for RecoveryAttempt {
    fn drop(&mut self) {
        self.finish(Route::HttpFallback, /*retry_after*/ None);
    }
}

/// Select the transport before the caller builds a request for it.
pub(super) fn select_for_request(
    session: &mut ModelClientSession,
    mode: &mut WebsocketRequestMode<'_>,
    params: &WebsocketConnectParams<'_>,
) -> Option<Connection> {
    let owner = session.client.websocket_routes.owner()?;
    let mut transport = owner.lock();
    match (&mut transport.route, mode) {
        (Route::Websocket, _) => Some(Connection::Websocket),
        (Route::Ready(socket), WebsocketRequestMode::Generate(request_transport)) => {
            let key = ResponsesConnectionKey::new(&params.api_provider, params.auth_revision);
            // A background socket has no outstanding requests. If it is closed or busy,
            // keep using HTTP rather than opening or waiting for another socket here.
            if socket.auth_owner_generation != params.auth_owner_generation
                || socket.auth_owner_generation != session.client.auth_owner_generation()
                || !socket.matches_setup(params, &key)
                || socket
                    .connection
                    .as_ref()
                    .is_none_or(|connection| connection.is_closed().now_or_never() != Some(false))
            {
                transport.route = Route::HttpFallback;
                return None;
            }
            if session.websocket_session.auth_owner_generation != socket.auth_owner_generation {
                session.turn_state = Arc::new(OnceLock::new());
            }
            session.websocket_session = std::mem::take(socket.as_mut());
            session
                .websocket_session
                .set_connection_reused(/*connection_reused*/ true);
            let (retry, failed_before_output) = oneshot::channel();
            **request_transport = ResponsesRequestTransport::Verifying(failed_before_output);
            transport.route = Route::Verifying;
            Some(Connection::Recovered(RecoveryAttempt {
                owner: Arc::downgrade(owner),
                retry_over_http: Some(retry),
            }))
        }
        _ => None,
    }
}

/// Start one background handshake alongside HTTP, without sending a prompt or a turn-state token.
pub(super) fn reconnect_if_due(
    client: &ModelClient,
    model_info: &ModelInfo,
    service_tier: Option<&str>,
    telemetry: &SessionTelemetry,
    metadata: &CodexResponsesMetadata,
) {
    let Some(owner) = client.websocket_routes.owner() else {
        return;
    };
    let mut transport = owner.lock();
    let now = Instant::now();
    match &transport.route {
        Route::HttpFallback => {}
        Route::Connecting(task) if task.is_finished() => {}
        _ => return,
    }
    if transport.next_probe.is_some_and(|at| now < at) {
        return;
    }
    transport.next_probe = Some(now + RETRY_INTERVAL);
    let route_owner = Arc::downgrade(owner);
    let client = ModelClient {
        websocket_routes: WebsocketRoutes::Probe,
        ..client.clone()
    };
    let model_info = model_info.clone();
    let service_tier = service_tier.map(str::to_owned);
    let telemetry = telemetry.clone();
    let metadata = metadata.clone();
    let timeout = client.state.provider.info().websocket_connect_timeout();
    let task = tokio::spawn(AuthStorageOriginator::current().scope(async move {
        let mut socket = WebsocketSession::default();
        let handshake = socket.preconnect(
            &client,
            WebsocketPreconnectMode::ProbeWithoutAuthRecovery,
            &model_info,
            service_tier,
            &telemetry,
            &metadata,
        );
        let result = tokio::time::timeout(timeout, handshake).await;
        let Some(owner) = route_owner.upgrade() else {
            return;
        };
        let mut transport = owner.lock();
        // A foreground 426 takes precedence over any in-flight probe.
        if !matches!(transport.route, Route::Connecting(_)) {
            return;
        }
        transport.route = match result {
            Ok(Ok(WebsocketHandshake::Connected)) => Route::Ready(Box::new(socket)),
            Ok(Ok(WebsocketHandshake::UpgradeRequired)) => Route::HttpOnly,
            Ok(Err(error)) => {
                transport.next_probe = transport
                    .next_probe
                    .max(error.retry_after().map(|advice| advice.deadline()));
                Route::HttpFallback
            }
            Err(_) => Route::HttpFallback,
        };
        tracing::debug!(
            name: "codex.websocket_probe_finished",
            ready = matches!(transport.route, Route::Ready(_)),
            "background WebSocket handshake finished"
        );
    }));
    transport.route = Route::Connecting(AbortOnDropHandle::new(task));
}
