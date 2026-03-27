use crate::transport::TransportEvent;
use crate::transport::remote_control::client_tracker::ClientTracker;
use crate::transport::remote_control::client_tracker::REMOTE_CONTROL_IDLE_SWEEP_INTERVAL;
use crate::transport::remote_control::enroll::RemoteControlConnectionAuth;
use crate::transport::remote_control::enroll::RemoteControlEnrollment;
use crate::transport::remote_control::enroll::enroll_remote_control_server;
use crate::transport::remote_control::enroll::format_headers;
use crate::transport::remote_control::enroll::load_persisted_remote_control_enrollment;
use crate::transport::remote_control::enroll::preview_remote_control_response_body;
use crate::transport::remote_control::enroll::update_persisted_remote_control_enrollment;

use super::protocol::ClientEnvelope;
use super::protocol::ClientEvent;
use super::protocol::ClientId;
use super::protocol::RemoteControlTarget;
use super::protocol::ServerEnvelope;
use axum::http::HeaderValue;
use base64::Engine;
use codex_core::AuthManager;
use codex_core::auth::UnauthorizedRecovery;
use codex_core::util::backoff;
use codex_state::StateRuntime;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::MissedTickBehavior;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

pub(super) const REMOTE_CONTROL_PROTOCOL_VERSION: &str = "2";
pub(super) const REMOTE_CONTROL_ACCOUNT_ID_HEADER: &str = "chatgpt-account-id";
const REMOTE_CONTROL_SUBSCRIBE_CURSOR_HEADER: &str = "x-codex-subscribe-cursor";

struct BoundedOutboundBuffer {
    buffer_by_client: HashMap<ClientId, BTreeMap<u64, ServerEnvelope>>,
    used_tx: watch::Sender<usize>,
}

impl BoundedOutboundBuffer {
    fn new() -> (Self, watch::Receiver<usize>) {
        let (used_tx, used_rx) = watch::channel(0);
        let buffer = Self {
            buffer_by_client: HashMap::new(),
            used_tx,
        };
        (buffer, used_rx)
    }

    fn insert(&mut self, server_envelope: &ServerEnvelope) {
        self.buffer_by_client
            .entry(server_envelope.client_id.clone())
            .or_default()
            .insert(server_envelope.seq_id, server_envelope.clone());
        self.used_tx.send_modify(|used| *used += 1);
    }

    fn remove(&mut self, client_id: &ClientId) {
        if let Some(buffer) = self.buffer_by_client.remove(client_id) {
            self.used_tx.send_modify(|used| *used -= buffer.len());
        }
    }

    fn ack(&mut self, client_id: &ClientId, acked_seq_id: u64) {
        let Some(buffer) = self.buffer_by_client.get_mut(client_id) else {
            return;
        };
        while let Some(seq_id) = buffer.first_key_value().map(|(seq_id, _)| seq_id)
            && *seq_id <= acked_seq_id
        {
            buffer.pop_first();
            self.used_tx.send_modify(|used| *used -= 1);
        }
        if buffer.is_empty() {
            self.buffer_by_client.remove(client_id);
        }
    }

    fn server_envelopes(&self) -> impl Iterator<Item = &ServerEnvelope> {
        self.buffer_by_client
            .values()
            .flat_map(|buffer| buffer.values())
    }
}

struct WebsocketState {
    outbound_buffer: BoundedOutboundBuffer,
    subscribe_cursor: Option<String>,
    next_seq_id: u64,
}

pub(crate) struct RemoteControlWebsocket {
    remote_control_target: RemoteControlTarget,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    shutdown_token: CancellationToken,
    reconnect_attempt: u64,
    enrollment: Option<RemoteControlEnrollment>,
    auth_recovery: UnauthorizedRecovery,
    client_tracker: Arc<Mutex<ClientTracker>>,
    state: Arc<Mutex<WebsocketState>>,
    server_event_rx: Arc<Mutex<mpsc::Receiver<super::QueuedServerEnvelope>>>,
    used_rx: watch::Receiver<usize>,
}

impl RemoteControlWebsocket {
    pub(crate) fn new(
        remote_control_target: RemoteControlTarget,
        state_db: Option<Arc<StateRuntime>>,
        auth_manager: Arc<AuthManager>,
        transport_event_tx: mpsc::Sender<TransportEvent>,
        shutdown_token: CancellationToken,
    ) -> Self {
        let shutdown_token = shutdown_token.child_token();
        let (server_event_tx, server_event_rx) = mpsc::channel(super::CHANNEL_CAPACITY);
        let client_tracker =
            ClientTracker::new(server_event_tx, transport_event_tx, &shutdown_token);
        let (outbound_buffer, used_rx) = BoundedOutboundBuffer::new();
        let auth_recovery = auth_manager.unauthorized_recovery();

        Self {
            remote_control_target,
            state_db,
            auth_manager,
            shutdown_token,
            reconnect_attempt: 0,
            enrollment: None,
            auth_recovery,
            client_tracker: Arc::new(Mutex::new(client_tracker)),
            state: Arc::new(Mutex::new(WebsocketState {
                outbound_buffer,
                subscribe_cursor: None,
                next_seq_id: 0,
            })),
            server_event_rx: Arc::new(Mutex::new(server_event_rx)),
            used_rx,
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            let shutdown_token = self.shutdown_token.child_token();
            let websocket_connection = match self.connect(&shutdown_token).await {
                Some(websocket_connection) => websocket_connection,
                None => break,
            };

            self.run_connection(websocket_connection, shutdown_token)
                .await;
        }

        self.client_tracker.lock().await.shutdown().await;
    }

    async fn connect(
        &mut self,
        shutdown_token: &CancellationToken,
    ) -> Option<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        loop {
            let subscribe_cursor = self.state.lock().await.subscribe_cursor.clone();
            tokio::select! {
                _ = shutdown_token.cancelled() => return None,
                connect_result = connect_remote_control_websocket(
                    &self.remote_control_target,
                    self.state_db.as_deref(),
                    &self.auth_manager,
                    &mut self.auth_recovery,
                    &mut self.enrollment,
                    subscribe_cursor.as_deref(),
                ) => {
                    match connect_result {
                        Ok((websocket_connection, response)) => {
                            self.reconnect_attempt = 0;
                            self.auth_recovery = self.auth_manager.unauthorized_recovery();
                            info!(
                                "connected to app-server remote control websocket: {}, {}",
                                self.remote_control_target.websocket_url,
                                format_headers(response.headers())
                            );
                            return Some(websocket_connection);
                        }
                        Err(err) => {
                            warn!("{err}");
                            let reconnect_delay = backoff(self.reconnect_attempt);
                            self.reconnect_attempt += 1;
                            tokio::select! {
                                _ = shutdown_token.cancelled() => return None,
                                _ = tokio::time::sleep(reconnect_delay) => {}
                            }
                        }
                    }
                }
            }
        }
    }

    async fn run_connection(
        &self,
        websocket_connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
        shutdown_token: CancellationToken,
    ) {
        let (websocket_writer, websocket_reader) = websocket_connection.split();
        let mut join_set = tokio::task::JoinSet::new();

        join_set.spawn(Self::run_server_writer(
            self.state.clone(),
            self.server_event_rx.clone(),
            self.used_rx.clone(),
            websocket_writer,
            shutdown_token.clone(),
        ));
        join_set.spawn(Self::run_websocket_reader(
            self.client_tracker.clone(),
            self.state.clone(),
            websocket_reader,
            shutdown_token.clone(),
        ));

        tokio::select! {
            _ = shutdown_token.cancelled() => {}
            _ = join_set.join_next() => shutdown_token.cancel(),
        }

        join_set.join_all().await;
    }

    async fn run_server_writer(
        state: Arc<Mutex<WebsocketState>>,
        server_event_rx: Arc<Mutex<mpsc::Receiver<super::QueuedServerEnvelope>>>,
        used_rx: watch::Receiver<usize>,
        websocket_writer: SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            tungstenite::Message,
        >,
        shutdown_token: CancellationToken,
    ) {
        let result = Self::run_server_writer_inner(
            state,
            server_event_rx,
            used_rx,
            websocket_writer,
            shutdown_token,
        )
        .await;
        if let Err(err) = result {
            warn!("remote control websocket writer disconnected, err: {err}");
        } else {
            warn!("remote control websocket writer was stopped");
        }
    }

    async fn run_server_writer_inner(
        state: Arc<Mutex<WebsocketState>>,
        server_event_rx: Arc<Mutex<mpsc::Receiver<super::QueuedServerEnvelope>>>,
        mut used_rx: watch::Receiver<usize>,
        mut websocket_writer: SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            tungstenite::Message,
        >,
        shutdown_token: CancellationToken,
    ) -> io::Result<()> {
        for server_envelope in state.lock().await.outbound_buffer.server_envelopes() {
            let payload = match serde_json::to_string(&server_envelope) {
                Ok(payload) => payload,
                Err(err) => {
                    error!("failed to serialize remote-control server event: {err}");
                    continue;
                }
            };
            tokio::select! {
                _ = shutdown_token.cancelled() => return Ok(()),
                send_result = websocket_writer.send(tungstenite::Message::Text(payload.into())) => {
                    if let Err(err) = send_result {
                        return Err(io::Error::other(err));
                    }
                }
            };
        }

        let mut server_event_rx = server_event_rx.lock().await;
        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => return Ok(()),
                _ = used_rx.wait_for(|used| *used < super::CHANNEL_CAPACITY) => {}
            };
            let queued_server_envelope = tokio::select! {
                _ = shutdown_token.cancelled() => return Ok(()),
                recv_result = server_event_rx.recv() => {
                    match recv_result {
                        Some(queued_server_envelope) => queued_server_envelope,
                        None => {
                            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "server event channel closed"));
                        }
                    }
                }
            };
            let (server_envelope, write_complete_tx) = {
                let mut state = state.lock().await;
                let seq_id = state.next_seq_id;
                state.next_seq_id = state.next_seq_id.saturating_add(1);

                let server_envelope = ServerEnvelope {
                    event: queued_server_envelope.event,
                    client_id: queued_server_envelope.client_id,
                    seq_id,
                };
                state.outbound_buffer.insert(&server_envelope);

                (server_envelope, queued_server_envelope.write_complete_tx)
            };

            let payload = match serde_json::to_string(&server_envelope) {
                Ok(payload) => payload,
                Err(err) => {
                    error!("failed to serialize remote-control server event: {err}");
                    continue;
                }
            };

            tokio::select! {
                _ = shutdown_token.cancelled() => return Ok(()),
                send_result = websocket_writer.send(tungstenite::Message::Text(payload.into())) => {
                    if let Err(err) = send_result {
                        return Err(io::Error::other(err));
                    }
                }
            };
            if let Some(write_complete_tx) = write_complete_tx {
                let _ = write_complete_tx.send(());
            }
        }
    }

    async fn run_websocket_reader(
        client_tracker: Arc<Mutex<ClientTracker>>,
        state: Arc<Mutex<WebsocketState>>,
        websocket_reader: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        shutdown_token: CancellationToken,
    ) {
        let result = Self::run_websocket_reader_inner(
            client_tracker,
            state,
            websocket_reader,
            shutdown_token,
        )
        .await;
        if let Err(err) = result {
            warn!("remote control websocket reader disconnected, err: {err}");
        } else {
            warn!("remote control websocket reader was stopped");
        }
    }

    async fn run_websocket_reader_inner(
        client_tracker: Arc<Mutex<ClientTracker>>,
        state: Arc<Mutex<WebsocketState>>,
        mut websocket_reader: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        shutdown_token: CancellationToken,
    ) -> io::Result<()> {
        let mut client_tracker = client_tracker.lock().await;
        let mut idle_sweep_interval = tokio::time::interval(REMOTE_CONTROL_IDLE_SWEEP_INTERVAL);
        idle_sweep_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            let incoming_message = tokio::select! {
                _ = shutdown_token.cancelled() => return Ok(()),
                client_id = client_tracker.bookkeep_join_set() => {
                    let Some(client_id) = client_id else {
                        continue;
                    };
                    if client_tracker.close_client(&client_id).await.is_err() {
                        return Ok(());
                    }
                    continue;
                }
                _ = idle_sweep_interval.tick() => {
                    let expired_client_ids = match client_tracker.close_expired_clients().await {
                        Ok(expired_client_ids) => expired_client_ids,
                        Err(_) => return Ok(()),
                    };
                    if !expired_client_ids.is_empty() {
                        let mut state = state.lock().await;
                        for client_id in expired_client_ids {
                            state.outbound_buffer.remove(&client_id);
                        }
                    }
                    continue;
                }
                incoming_message = websocket_reader.next() => {
                    match incoming_message {
                        Some(incoming_message) => incoming_message,
                        None => return Err(io::Error::new(ErrorKind::UnexpectedEof, "websocket stream ended")),
                    }
                }
            };
            let client_envelope = match incoming_message {
                Ok(tungstenite::Message::Text(text)) => {
                    match serde_json::from_str::<ClientEnvelope>(&text) {
                        Ok(client_envelope) => client_envelope,
                        Err(err) => {
                            warn!("failed to deserialize remote-control client event: {err}");
                            continue;
                        }
                    }
                }
                Ok(tungstenite::Message::Ping(_))
                | Ok(tungstenite::Message::Pong(_))
                | Ok(tungstenite::Message::Frame(_)) => continue,
                Ok(tungstenite::Message::Binary(_)) => {
                    warn!("dropping unsupported binary remote-control websocket message");
                    continue;
                }
                Ok(tungstenite::Message::Close(_)) => {
                    return Err(io::Error::new(
                        ErrorKind::ConnectionAborted,
                        "websocket disconnected",
                    ));
                }
                Err(err) => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!("failed to read from websocket: {err}"),
                    ));
                }
            };

            let mut state = state.lock().await;
            if let Some(cursor) = client_envelope.cursor.as_deref() {
                state.subscribe_cursor = Some(cursor.to_string());
            }
            if let ClientEvent::Ack = &client_envelope.event
                && let Some(acked_seq_id) = client_envelope.seq_id
            {
                state
                    .outbound_buffer
                    .ack(&client_envelope.client_id, acked_seq_id);
            }
            if matches!(&client_envelope.event, ClientEvent::ClientClosed)
                || remote_control_message_starts_connection(&client_envelope.event)
            {
                state.outbound_buffer.remove(&client_envelope.client_id);
            }
            drop(state);

            if client_tracker
                .handle_message(client_envelope)
                .await
                .is_err()
            {
                return Ok(());
            }
        }
    }
}

fn remote_control_message_starts_connection(event: &ClientEvent) -> bool {
    matches!(
        event,
        ClientEvent::ClientMessage {
            message: codex_app_server_protocol::JSONRPCMessage::Request(
                codex_app_server_protocol::JSONRPCRequest { method, .. }
            ),
        } if method == "initialize"
    )
}

fn set_remote_control_header(
    headers: &mut tungstenite::http::HeaderMap,
    name: &'static str,
    value: &str,
) -> io::Result<()> {
    let header_value = HeaderValue::from_str(value).map_err(|err| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid remote control header `{name}`: {err}"),
        )
    })?;
    headers.insert(name, header_value);
    Ok(())
}

fn build_remote_control_websocket_request(
    websocket_url: &str,
    enrollment: &RemoteControlEnrollment,
    auth: &RemoteControlConnectionAuth,
    subscribe_cursor: Option<&str>,
) -> io::Result<tungstenite::http::Request<()>> {
    let mut request = websocket_url.into_client_request().map_err(|err| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid remote control websocket URL `{websocket_url}`: {err}"),
        )
    })?;
    let headers = request.headers_mut();
    set_remote_control_header(headers, "x-codex-server-id", &enrollment.server_id)?;
    set_remote_control_header(
        headers,
        "x-codex-name",
        &base64::engine::general_purpose::STANDARD.encode(&enrollment.server_name),
    )?;
    set_remote_control_header(
        headers,
        "x-codex-protocol-version",
        REMOTE_CONTROL_PROTOCOL_VERSION,
    )?;
    set_remote_control_header(
        headers,
        "authorization",
        &format!("Bearer {}", auth.bearer_token),
    )?;
    if let Some(account_id) = auth.account_id.as_deref() {
        set_remote_control_header(headers, REMOTE_CONTROL_ACCOUNT_ID_HEADER, account_id)?;
    }
    if let Some(subscribe_cursor) = subscribe_cursor {
        set_remote_control_header(
            headers,
            REMOTE_CONTROL_SUBSCRIBE_CURSOR_HEADER,
            subscribe_cursor,
        )?;
    }
    Ok(request)
}

pub(crate) async fn load_remote_control_auth(
    auth_manager: &Arc<AuthManager>,
) -> io::Result<RemoteControlConnectionAuth> {
    let auth = match auth_manager.auth().await {
        Some(auth) => auth,
        None => {
            auth_manager.reload();
            auth_manager.auth().await.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::PermissionDenied,
                    "remote control requires ChatGPT authentication",
                )
            })?
        }
    };

    if !auth.is_chatgpt_auth() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "remote control requires ChatGPT authentication; API key auth is not supported",
        ));
    }

    Ok(RemoteControlConnectionAuth {
        bearer_token: auth.get_token().map_err(io::Error::other)?,
        account_id: auth.get_account_id(),
    })
}

pub(super) async fn connect_remote_control_websocket(
    remote_control_target: &RemoteControlTarget,
    state_db: Option<&StateRuntime>,
    auth_manager: &Arc<AuthManager>,
    auth_recovery: &mut UnauthorizedRecovery,
    enrollment: &mut Option<RemoteControlEnrollment>,
    subscribe_cursor: Option<&str>,
) -> io::Result<(
    WebSocketStream<MaybeTlsStream<TcpStream>>,
    tungstenite::http::Response<()>,
)> {
    ensure_rustls_crypto_provider();

    let auth = load_remote_control_auth(auth_manager).await?;
    if auth.account_id.as_ref()
        != enrollment
            .as_ref()
            .and_then(|enrollment| enrollment.account_id.as_ref())
    {
        *enrollment = None;
    }

    if enrollment.is_none() {
        *enrollment = load_persisted_remote_control_enrollment(
            state_db,
            remote_control_target,
            auth.account_id.as_deref(),
        )
        .await;
    }

    if enrollment.is_none() {
        let new_enrollment = match enroll_remote_control_server(remote_control_target, &auth).await
        {
            Ok(new_enrollment) => new_enrollment,
            Err(err)
                if err.kind() == ErrorKind::PermissionDenied
                    && recover_remote_control_auth(auth_recovery).await =>
            {
                return Err(io::Error::other(format!(
                    "{err}; retrying after auth recovery"
                )));
            }
            Err(err) => return Err(err),
        };
        if let Err(err) = update_persisted_remote_control_enrollment(
            state_db,
            remote_control_target,
            auth.account_id.as_deref(),
            Some(&new_enrollment),
        )
        .await
        {
            warn!("failed to persist remote control enrollment in sqlite state db: {err}");
        }
        *enrollment = Some(new_enrollment);
    }

    let enrollment_ref = enrollment.as_ref().ok_or_else(|| {
        io::Error::other("missing remote control enrollment after enrollment step")
    })?;
    let request = build_remote_control_websocket_request(
        &remote_control_target.websocket_url,
        enrollment_ref,
        &auth,
        subscribe_cursor,
    )?;

    match connect_async(request).await {
        Ok((websocket_stream, response)) => Ok((websocket_stream, response.map(|_| ()))),
        Err(err) => {
            match &err {
                tungstenite::Error::Http(response) if response.status().as_u16() == 404 => {
                    if let Err(clear_err) = update_persisted_remote_control_enrollment(
                        state_db,
                        remote_control_target,
                        auth.account_id.as_deref(),
                        /*enrollment*/ None,
                    )
                    .await
                    {
                        warn!(
                            "failed to clear stale remote control enrollment in sqlite state db: {clear_err}"
                        );
                    }
                    *enrollment = None;
                }
                tungstenite::Error::Http(response)
                    if matches!(response.status().as_u16(), 401 | 403) =>
                {
                    if recover_remote_control_auth(auth_recovery).await {
                        return Err(io::Error::other(format!(
                            "remote control websocket auth failed with HTTP {}; retrying after auth recovery",
                            response.status()
                        )));
                    }
                }
                _ => {}
            }
            Err(io::Error::other(
                format_remote_control_websocket_connect_error(
                    &remote_control_target.websocket_url,
                    &err,
                ),
            ))
        }
    }
}

async fn recover_remote_control_auth(auth_recovery: &mut UnauthorizedRecovery) -> bool {
    if !auth_recovery.has_next() {
        return false;
    }

    let mode = auth_recovery.mode_name();
    let step = auth_recovery.step_name();
    match auth_recovery.next().await {
        Ok(step_result) => {
            info!(
                "remote control websocket auth recovery succeeded: mode={mode}, step={step}, auth_state_changed={:?}",
                step_result.auth_state_changed()
            );
            true
        }
        Err(err) => {
            warn!("remote control websocket auth recovery failed: mode={mode}, step={step}: {err}");
            false
        }
    }
}

fn format_remote_control_websocket_connect_error(
    websocket_url: &str,
    err: &tungstenite::Error,
) -> String {
    let mut message =
        format!("failed to connect app-server remote control websocket `{websocket_url}`: {err}");
    let tungstenite::Error::Http(response) = err else {
        return message;
    };

    message.push_str(&format!(", {}", format_headers(response.headers())));
    if let Some(body) = response.body().as_ref()
        && !body.is_empty()
    {
        let body_preview = preview_remote_control_response_body(body);
        message.push_str(&format!(", body: {body_preview}"));
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::remote_control::protocol::normalize_remote_control_url;
    use chrono::Utc;
    use codex_app_server_protocol::AuthMode;
    use codex_core::CodexAuth;
    use codex_core::auth::AuthCredentialsStoreMode;
    use codex_core::auth::AuthDotJson;
    use codex_core::auth::save_auth;
    use codex_core::test_support::auth_manager_from_auth;
    use codex_login::token_data::TokenData;
    use codex_login::token_data::parse_chatgpt_jwt_claims;
    use codex_state::StateRuntime;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;
    use tokio::sync::mpsc;
    use tokio::time::Duration;
    use tokio::time::timeout;

    async fn remote_control_state_runtime(codex_home: &TempDir) -> Arc<StateRuntime> {
        StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string())
            .await
            .expect("state runtime should initialize")
    }

    fn remote_control_auth_manager() -> Arc<AuthManager> {
        auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
    }

    fn remote_control_url_for_listener(listener: &TcpListener) -> String {
        format!(
            "http://localhost:{}/backend-api/",
            listener
                .local_addr()
                .expect("listener should have a local addr")
                .port()
        )
    }

    fn remote_control_auth_dot_json(access_token: &str) -> AuthDotJson {
        #[derive(serde::Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }

        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_user_id": "user-12345",
                "user_id": "user-12345",
                "chatgpt_account_id": "account_id"
            }
        });
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let header_b64 = b64(&serde_json::to_vec(&header).expect("header should serialize"));
        let payload_b64 = b64(&serde_json::to_vec(&payload).expect("payload should serialize"));
        let fake_jwt = format!("{header_b64}.{payload_b64}.sig");

        AuthDotJson {
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: parse_chatgpt_jwt_claims(&fake_jwt).expect("fake jwt should parse"),
                access_token: access_token.to_string(),
                refresh_token: "refresh-token".to_string(),
                account_id: Some("account_id".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        }
    }

    #[tokio::test]
    async fn connect_remote_control_websocket_includes_http_error_details() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = remote_control_url_for_listener(&listener);
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let expected_error = format!(
            "failed to connect app-server remote control websocket `{}`: HTTP error: 503 Service Unavailable, request-id: <none>, cf-ray: <none>, body: upstream unavailable",
            remote_control_target.websocket_url
        );
        let server_task = tokio::spawn(async move {
            let (stream, request_line) = accept_http_request(&listener).await;
            assert_eq!(
                request_line,
                "GET /backend-api/wham/remote/control/server HTTP/1.1"
            );
            respond_with_status_and_headers(
                stream,
                "503 Service Unavailable",
                &[("x-trace-id", "trace-503"), ("x-region", "us-east-1")],
                "upstream unavailable",
            )
            .await;
        });
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_db = remote_control_state_runtime(&codex_home).await;
        let auth_manager = remote_control_auth_manager();
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        let mut enrollment = Some(RemoteControlEnrollment {
            account_id: Some("account_id".to_string()),
            server_id: "srv_e_test".to_string(),
            server_name: "test-server".to_string(),
        });

        let err = match connect_remote_control_websocket(
            &remote_control_target,
            Some(state_db.as_ref()),
            &auth_manager,
            &mut auth_recovery,
            &mut enrollment,
            None,
        )
        .await
        {
            Ok(_) => panic!("http error response should fail the websocket connect"),
            Err(err) => err,
        };

        server_task.await.expect("server task should succeed");
        assert_eq!(err.to_string(), expected_error);
    }

    #[tokio::test]
    async fn connect_remote_control_websocket_recovers_after_unauthorized_reload() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = remote_control_url_for_listener(&listener);
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let server_task = tokio::spawn(async move {
            let (stream, request_line) = accept_http_request(&listener).await;
            assert_eq!(
                request_line,
                "GET /backend-api/wham/remote/control/server HTTP/1.1"
            );
            respond_with_status_and_headers(stream, "401 Unauthorized", &[], "unauthorized").await;
        });
        let codex_home = TempDir::new().expect("temp dir should create");
        save_auth(
            codex_home.path(),
            &remote_control_auth_dot_json("stale-token"),
            AuthCredentialsStoreMode::File,
        )
        .expect("stale auth should save");
        let state_db = remote_control_state_runtime(&codex_home).await;
        let auth_manager = AuthManager::shared(
            codex_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
        );
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        let mut enrollment = Some(RemoteControlEnrollment {
            account_id: Some("account_id".to_string()),
            server_id: "srv_e_test".to_string(),
            server_name: "test-server".to_string(),
        });
        save_auth(
            codex_home.path(),
            &remote_control_auth_dot_json("fresh-token"),
            AuthCredentialsStoreMode::File,
        )
        .expect("fresh auth should save");

        let err = connect_remote_control_websocket(
            &remote_control_target,
            Some(state_db.as_ref()),
            &auth_manager,
            &mut auth_recovery,
            &mut enrollment,
            None,
        )
        .await
        .expect_err("unauthorized response should fail the websocket connect");

        server_task.await.expect("server task should succeed");
        assert_eq!(
            err.to_string(),
            "remote control websocket auth failed with HTTP 401 Unauthorized; retrying after auth recovery"
        );
        assert_eq!(
            auth_manager
                .auth()
                .await
                .expect("auth should remain available")
                .get_token()
                .expect("token should be readable"),
            "fresh-token"
        );
    }

    #[tokio::test]
    async fn connect_remote_control_websocket_recovers_after_unauthorized_enrollment() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = remote_control_url_for_listener(&listener);
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let enroll_url = remote_control_target.enroll_url.clone();
        let server_task = tokio::spawn(async move {
            let (stream, request_line) = accept_http_request(&listener).await;
            assert_eq!(
                request_line,
                "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
            );
            respond_with_status_and_headers(stream, "401 Unauthorized", &[], "unauthorized").await;
        });
        let codex_home = TempDir::new().expect("temp dir should create");
        save_auth(
            codex_home.path(),
            &remote_control_auth_dot_json("stale-token"),
            AuthCredentialsStoreMode::File,
        )
        .expect("stale auth should save");
        let state_db = remote_control_state_runtime(&codex_home).await;
        let auth_manager = AuthManager::shared(
            codex_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
        );
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        let mut enrollment = None;
        save_auth(
            codex_home.path(),
            &remote_control_auth_dot_json("fresh-token"),
            AuthCredentialsStoreMode::File,
        )
        .expect("fresh auth should save");

        let err = connect_remote_control_websocket(
            &remote_control_target,
            Some(state_db.as_ref()),
            &auth_manager,
            &mut auth_recovery,
            &mut enrollment,
            None,
        )
        .await
        .expect_err("unauthorized enrollment should fail the websocket connect");

        server_task.await.expect("server task should succeed");
        assert_eq!(
            err.to_string(),
            format!(
                "remote control server enrollment failed at `{enroll_url}`: HTTP 401 Unauthorized, request-id: <none>, cf-ray: <none>, body: unauthorized; retrying after auth recovery"
            )
        );
        assert_eq!(
            auth_manager
                .auth()
                .await
                .expect("auth should remain available")
                .get_token()
                .expect("token should be readable"),
            "fresh-token"
        );
    }

    #[tokio::test]
    async fn run_remote_control_websocket_loop_shutdown_cancels_reconnect_backoff() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = remote_control_url_for_listener(&listener);
        drop(listener);

        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let (transport_event_tx, transport_event_rx) = mpsc::channel(1);
        drop(transport_event_rx);
        let shutdown_token = CancellationToken::new();
        let websocket_task = tokio::spawn({
            let shutdown_token = shutdown_token.clone();
            async move {
                RemoteControlWebsocket::new(
                    remote_control_target,
                    None,
                    remote_control_auth_manager(),
                    transport_event_tx,
                    shutdown_token,
                )
                .run()
                .await
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown_token.cancel();

        timeout(Duration::from_millis(100), websocket_task)
            .await
            .expect("shutdown should cancel reconnect backoff")
            .expect("websocket task should join");
    }

    async fn accept_http_request(listener: &TcpListener) -> (TcpStream, String) {
        let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("HTTP request should arrive in time")
            .expect("listener accept should succeed");
        let mut reader = BufReader::new(stream);

        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .await
            .expect("request line should read");
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("header line should read");
            if line == "\r\n" {
                break;
            }
        }

        (
            reader.into_inner(),
            request_line.trim_end_matches("\r\n").to_string(),
        )
    }

    async fn respond_with_status_and_headers(
        mut stream: TcpStream,
        status: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) {
        let extra_headers = headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n{body}",
            body.len(),
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
        stream.flush().await.expect("response should flush");
    }
}
