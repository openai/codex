use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout_at;

use super::ConnectionStatus;
use super::ExecServerClient;
use super::ExecServerError;
use super::Inner;
use super::OrderedSessionEvents;
use super::SessionState;
use super::disconnected_message;
use super::fail_all_in_flight_work;
use super::handle_server_notification;
use super::is_transport_closed_error;
use crate::process::ExecProcessEvent;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::rpc::RpcClient;
use crate::rpc::RpcClientEvent;
use crate::rpc::SESSION_ALREADY_ATTACHED_ERROR_CODE;

#[cfg(test)]
const SESSION_RECOVERY_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(not(test))]
// Leave margin inside the server's 30-second retention windows because the
// client and server start their disconnect clocks independently.
const SESSION_RECOVERY_TIMEOUT: Duration = Duration::from_secs(25);
const SESSION_RECOVERY_RETRY_INTERVAL: Duration = Duration::from_millis(100);
const REGISTRY_RECOVERY_INITIAL_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const REGISTRY_RECOVERY_MAX_RETRY_INTERVAL: Duration = Duration::from_secs(5);

impl SessionState {
    fn last_published_seq(&self) -> u64 {
        self.ordered_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last_published_seq
    }

    fn recover_events(&self, response: ReadResponse) -> Result<bool, ExecServerError> {
        let ReadResponse {
            chunks,
            next_seq,
            exited,
            exit_code,
            closed,
            failure,
        } = response;
        if let Some(message) = failure {
            return Err(ExecServerError::Protocol(format!(
                "process failed while recovering: {message}"
            )));
        }

        let target_seq = next_seq.saturating_sub(1);
        let published_closed = {
            let mut ordered_events = self
                .ordered_events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if ordered_events.failure.is_some()
                || ordered_events.closed_published
                || target_seq <= ordered_events.last_published_seq
            {
                return Ok(false);
            }
            for chunk in chunks {
                if chunk.seq > ordered_events.last_published_seq {
                    ordered_events
                        .pending
                        .entry(chunk.seq)
                        .or_insert(ExecProcessEvent::Output(chunk));
                }
            }
            if closed {
                match ordered_events.pending.get(&target_seq) {
                    Some(ExecProcessEvent::Closed { .. }) => {}
                    Some(_) => {
                        return Err(ExecServerError::Protocol(format!(
                            "process close sequence {target_seq} conflicts with recovered output"
                        )));
                    }
                    None => {
                        ordered_events
                            .pending
                            .insert(target_seq, ExecProcessEvent::Closed { seq: target_seq });
                    }
                }
            }

            let exit_known = ordered_events.exit_published
                || ordered_events
                    .pending
                    .range(..=target_seq)
                    .any(|(_, event)| matches!(event, ExecProcessEvent::Exited { .. }));
            let event_count = target_seq - ordered_events.last_published_seq;
            let retained_count = ordered_events
                .pending
                .range(ordered_events.last_published_seq.saturating_add(1)..=target_seq)
                .count() as u64;
            let missing_count = event_count.saturating_sub(retained_count);
            if exited && !exit_known {
                if missing_count != 1 {
                    return Err(recovery_gap_error(target_seq));
                }
                let seq = first_missing_seq(&ordered_events, target_seq);
                let exit_code = exit_code.ok_or_else(|| {
                    ExecServerError::Protocol(
                        "recovering exited process did not include its exit code".to_string(),
                    )
                })?;
                ordered_events
                    .pending
                    .insert(seq, ExecProcessEvent::Exited { seq, exit_code });
            } else if missing_count != 0 {
                return Err(recovery_gap_error(target_seq));
            }
            self.publish_ready(&mut ordered_events)
        };

        self.note_change(target_seq);
        Ok(published_closed)
    }
}

fn first_missing_seq(events: &OrderedSessionEvents, target_seq: u64) -> u64 {
    let mut expected = events.last_published_seq.saturating_add(1);
    for seq in events
        .pending
        .range(expected..=target_seq)
        .map(|(seq, _)| *seq)
    {
        if seq != expected {
            break;
        }
        expected = expected.saturating_add(1);
    }
    expected
}

fn recovery_gap_error(target_seq: u64) -> ExecServerError {
    ExecServerError::Protocol(format!(
        "process events are no longer retained while recovering through sequence {target_seq}"
    ))
}

impl Inner {
    pub(super) async fn rpc_client(self: &Arc<Self>) -> Result<Arc<RpcClient>, ExecServerError> {
        let mut connection_changed = self.connection_changed.subscribe();
        loop {
            if let Some(message) = self.failure_message() {
                return Err(ExecServerError::Disconnected(message));
            }

            let rpc_client = {
                let connection = self
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &connection.status {
                    ConnectionStatus::Connected(rpc_client) => Some(Arc::clone(rpc_client)),
                    ConnectionStatus::Recovering | ConnectionStatus::Failed(_) => None,
                }
            };
            let Some(rpc_client) = rpc_client else {
                let _ = connection_changed.changed().await;
                continue;
            };
            if !rpc_client.is_disconnected() {
                return Ok(rpc_client);
            }

            let _ = connection_changed.changed().await;
        }
    }

    pub(super) fn begin_process_start(&self, expected: &Arc<RpcClient>) -> bool {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ConnectionStatus::Connected(current) = &connection.status else {
            return false;
        };
        if !Arc::ptr_eq(current, expected) || expected.is_disconnected() {
            return false;
        }
        connection.active_process_starts += 1;
        true
    }

    pub(super) fn finish_process_start(&self) {
        {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if connection.active_process_starts == 0 {
                tracing::error!("finished an exec-server process start that was not active");
                return;
            }
            connection.active_process_starts -= 1;
        }
        self.notify_connection_changed();
    }

    pub(super) fn is_failed(&self) -> bool {
        self.failure_message().is_some()
    }

    pub(super) fn failure_message(&self) -> Option<String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &connection.status {
            ConnectionStatus::Failed(message) => Some(message.clone()),
            ConnectionStatus::Connected(_) | ConnectionStatus::Recovering => None,
        }
    }

    fn request_recovery(
        self: &Arc<Self>,
        failed_rpc_client: Arc<RpcClient>,
        disconnect_message: String,
    ) {
        let should_recover = {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match &connection.status {
                ConnectionStatus::Connected(current)
                    if Arc::ptr_eq(current, &failed_rpc_client) =>
                {
                    connection.status = ConnectionStatus::Recovering;
                    true
                }
                ConnectionStatus::Connected(_)
                | ConnectionStatus::Recovering
                | ConnectionStatus::Failed(_) => false,
            }
        };
        if !should_recover {
            return;
        }

        self.notify_connection_changed();
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            inner.recover(disconnect_message).await;
        });
    }

    async fn recover(self: &Arc<Self>, disconnect_message: String) {
        let deadline = Instant::now() + SESSION_RECOVERY_TIMEOUT;
        self.fail_all_http_body_streams(disconnect_message.clone())
            .await;
        if timeout_at(deadline, self.wait_for_process_starts())
            .await
            .is_err()
        {
            let message = format!(
                "{disconnect_message}; failed to resume exec-server session: recovery timed out after {SESSION_RECOVERY_TIMEOUT:?}"
            );
            self.fail(message).await;
            return;
        }
        if self.reconnect_strategy.is_none() {
            self.fail(disconnect_message).await;
            return;
        }

        let Some(session_id) = self.session_id.get().cloned() else {
            let message = format!(
                "{disconnect_message}; failed to resume exec-server session: missing session id"
            );
            self.fail(message).await;
            return;
        };
        let mut registry_retry_attempt = 0;
        let last_error = loop {
            let retry_delay = match timeout_at(deadline, self.resume_once(&session_id)).await {
                Ok(Ok(candidate)) if !candidate.is_disconnected() => {
                    if self.install_recovered_client(candidate) {
                        return;
                    }
                    registry_retry_attempt = 0;
                    SESSION_RECOVERY_RETRY_INTERVAL
                }
                Ok(Ok(_)) => {
                    registry_retry_attempt = 0;
                    SESSION_RECOVERY_RETRY_INTERVAL
                }
                Ok(Err(error)) if !is_retryable_recovery_error(&error) => {
                    break error.to_string();
                }
                Ok(Err(error)) => {
                    if is_retryable_registry_error(&error) {
                        let delay = registry_recovery_retry_delay(
                            &error,
                            &session_id,
                            registry_retry_attempt,
                        );
                        registry_retry_attempt = registry_retry_attempt.saturating_add(1);
                        delay
                    } else {
                        registry_retry_attempt = 0;
                        SESSION_RECOVERY_RETRY_INTERVAL
                    }
                }
                Err(_) => {
                    break format!("recovery timed out after {SESSION_RECOVERY_TIMEOUT:?}");
                }
            };

            let now = Instant::now();
            if now >= deadline {
                break format!("recovery timed out after {SESSION_RECOVERY_TIMEOUT:?}");
            }
            sleep(retry_delay.min(deadline - now)).await;
        };

        let message =
            format!("{disconnect_message}; failed to resume exec-server session: {last_error}");
        self.fail(message).await;
    }

    async fn wait_for_process_starts(&self) {
        let mut connection_changed = self.connection_changed.subscribe();
        loop {
            let starts_are_done = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .active_process_starts
                == 0;
            if starts_are_done {
                return;
            }
            let _ = connection_changed.changed().await;
        }
    }

    fn install_recovered_client(&self, rpc_client: Arc<RpcClient>) -> bool {
        let installed = {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !matches!(connection.status, ConnectionStatus::Recovering)
                || rpc_client.is_disconnected()
            {
                false
            } else {
                connection.status = ConnectionStatus::Connected(rpc_client);
                true
            }
        };
        if installed {
            self.notify_connection_changed();
        }
        installed
    }

    fn notify_connection_changed(&self) {
        self.connection_changed.send_replace(());
    }

    async fn resume_once(
        self: &Arc<Self>,
        session_id: &str,
    ) -> Result<Arc<RpcClient>, ExecServerError> {
        let reconnect_strategy = self
            .reconnect_strategy
            .as_ref()
            .ok_or_else(|| ExecServerError::Protocol("missing reconnect strategy".to_string()))?;
        let (connection, options) = reconnect_strategy.resume(session_id).await?;
        let (rpc_client, events_rx) = RpcClient::new(connection);
        let rpc_client = Arc::new(rpc_client);
        let client = ExecServerClient {
            inner: Arc::clone(self),
        };
        // Resuming a session redirects notifications from its running processes
        // to this connection during initialize. Drain them immediately so a
        // burst cannot fill the bounded event channel and block the initialize
        // response behind it.
        client.spawn_rpc_reader(&rpc_client, events_rx);
        client.initialize_rpc(&rpc_client, options).await?;

        self.recover_processes(&rpc_client).await?;
        Ok(rpc_client)
    }

    async fn recover_processes(
        self: &Arc<Self>,
        rpc_client: &RpcClient,
    ) -> Result<(), ExecServerError> {
        let sessions = self.sessions.load_full();
        for (process_id, session) in sessions.iter() {
            if !session.recoverable.load(Ordering::Acquire) {
                continue;
            }
            let response = rpc_client
                .call::<_, ReadResponse>(
                    EXEC_READ_METHOD,
                    &ReadParams {
                        process_id: process_id.clone(),
                        after_seq: Some(session.last_published_seq()),
                        max_bytes: None,
                        wait_ms: Some(0),
                    },
                )
                .await
                .map_err(ExecServerError::from);
            let recovered = match response {
                Ok(response) => session.recover_events(response),
                Err(error) if is_transport_closed_error(&error) => return Err(error),
                Err(error) => Err(error),
            };
            match recovered {
                Ok(true) => self.remove_session_if(process_id, session),
                Ok(false) => {}
                Err(error) => {
                    let terminated: Result<TerminateResponse, ExecServerError> = rpc_client
                        .call(
                            EXEC_TERMINATE_METHOD,
                            &TerminateParams {
                                process_id: process_id.clone(),
                            },
                        )
                        .await
                        .map_err(ExecServerError::from);
                    if let Err(terminate_error) = terminated
                        && is_transport_closed_error(&terminate_error)
                    {
                        return Err(terminate_error);
                    }
                    self.remove_session_if(process_id, session);
                    session.set_failure(format!("failed to recover process {process_id}: {error}"));
                }
            }
        }
        Ok(())
    }

    async fn fail(self: &Arc<Self>, message: String) {
        let (message, newly_failed) = {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match &connection.status {
                ConnectionStatus::Failed(existing) => (existing.clone(), false),
                ConnectionStatus::Connected(_) | ConnectionStatus::Recovering => {
                    connection.status = ConnectionStatus::Failed(message.clone());
                    (message, true)
                }
            }
        };
        if newly_failed {
            self.notify_connection_changed();
            fail_all_in_flight_work(self, message.clone()).await;
        }
    }
}

impl ExecServerClient {
    pub(super) fn spawn_rpc_reader(
        &self,
        rpc_client: &Arc<RpcClient>,
        mut events_rx: mpsc::Receiver<RpcClientEvent>,
    ) {
        let inner = Arc::downgrade(&self.inner);
        let rpc_client = Arc::downgrade(rpc_client);
        tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                let (Some(inner), Some(rpc_client)) = (inner.upgrade(), rpc_client.upgrade())
                else {
                    return;
                };
                match event {
                    RpcClientEvent::Notification(notification) => {
                        if let Err(error) = handle_server_notification(&inner, notification).await {
                            rpc_client.close_transport().await;
                            inner.request_recovery(
                                rpc_client,
                                format!("exec-server notification handling failed: {error}"),
                            );
                            return;
                        }
                    }
                    RpcClientEvent::Disconnected { reason } => {
                        inner.request_recovery(rpc_client, disconnected_message(reason.as_deref()));
                        return;
                    }
                }
            }
        });
    }
}

fn is_retryable_recovery_error(error: &ExecServerError) -> bool {
    is_transport_closed_error(error)
        || matches!(
            error,
            ExecServerError::WebSocketConnectTimeout { .. }
                | ExecServerError::WebSocketConnect { .. }
                | ExecServerError::InitializeTimedOut { .. }
        )
        || is_retryable_registry_error(error)
        || matches!(
            error,
            ExecServerError::Server { code, .. }
                if *code == SESSION_ALREADY_ATTACHED_ERROR_CODE
        )
}

fn is_retryable_registry_error(error: &ExecServerError) -> bool {
    matches!(
        error,
        ExecServerError::EnvironmentRegistryRequest(error)
            if error.is_connect() || error.is_timeout()
    ) || matches!(
        error,
        ExecServerError::EnvironmentRegistryHttp { status, .. }
            if status.is_server_error()
                || *status == reqwest::StatusCode::REQUEST_TIMEOUT
                || *status == reqwest::StatusCode::TOO_MANY_REQUESTS
    )
}

fn registry_recovery_retry_delay(
    error: &ExecServerError,
    session_id: &str,
    attempt: u32,
) -> Duration {
    let multiplier = 1_u32.checked_shl(attempt.min(4)).unwrap_or(u32::MAX);
    let base_delay = REGISTRY_RECOVERY_INITIAL_RETRY_INTERVAL
        .saturating_mul(multiplier)
        .min(REGISTRY_RECOVERY_MAX_RETRY_INTERVAL);
    let base_millis = base_delay.as_millis() as u64;
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    attempt.hash(&mut hasher);
    let jittered_delay =
        Duration::from_millis(base_millis / 2 + hasher.finish() % (base_millis / 2 + 1));
    let retry_after = if let ExecServerError::EnvironmentRegistryHttp { retry_after, .. } = error {
        *retry_after
    } else {
        None
    };
    retry_after.map_or(jittered_delay, |delay| delay.max(jittered_delay))
}
