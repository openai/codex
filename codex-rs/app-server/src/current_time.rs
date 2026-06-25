use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::CurrentTimeReadParams;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::CurrentTimeSleepNotification;
use codex_app_server_protocol::CurrentTimeWakeParams;
use codex_app_server_protocol::CurrentTimeWakeResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_core::SleepFuture;
use codex_core::TimeFuture;
use codex_core::TimeProvider;
use codex_protocol::ThreadId;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout_at;
use uuid::Uuid;

use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::thread_state::ThreadStateManager;

const EXTERNAL_CLOCK_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn app_server_time_provider(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
) -> Arc<AppServerTimeProvider> {
    Arc::new(AppServerTimeProvider {
        outgoing: Arc::downgrade(&outgoing),
        thread_state_manager,
        pending_sleeps: Arc::new(PendingSleeps::default()),
    })
}

pub(crate) struct AppServerTimeProvider {
    outgoing: Weak<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    pending_sleeps: Arc<PendingSleeps>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct SleepKey {
    thread_id: String,
    sleep_id: String,
}

#[derive(Default)]
struct PendingSleeps {
    // Dropping a canceled sleep future removes its waiter synchronously.
    wake_senders: Mutex<HashMap<SleepKey, oneshot::Sender<()>>>,
}

impl PendingSleeps {
    fn register(&self, key: SleepKey) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.wake_senders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, tx);
        rx
    }

    fn wake(&self, key: &SleepKey) {
        let sender = self
            .wake_senders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
        if let Some(sender) = sender {
            let _ = sender.send(());
        }
    }

    fn remove(&self, key: &SleepKey) {
        self.wake_senders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
    }
}

struct PendingSleepGuard {
    pending_sleeps: Arc<PendingSleeps>,
    key: SleepKey,
}

impl Drop for PendingSleepGuard {
    fn drop(&mut self) {
        self.pending_sleeps.remove(&self.key);
    }
}

impl AppServerTimeProvider {
    pub(crate) fn wake(&self, params: CurrentTimeWakeParams) -> CurrentTimeWakeResponse {
        self.pending_sleeps.wake(&SleepKey {
            thread_id: params.thread_id,
            sleep_id: params.sleep_id,
        });
        CurrentTimeWakeResponse {}
    }
}

impl TimeProvider for AppServerTimeProvider {
    fn current_time(&self, thread_id: ThreadId) -> TimeFuture<'_> {
        let outgoing = self.outgoing.clone();
        let thread_state_manager = self.thread_state_manager.clone();
        Box::pin(async move {
            let outgoing = outgoing
                .upgrade()
                .context("app-server current-time provider is unavailable")?;
            request_current_time(outgoing, thread_state_manager, thread_id).await
        })
    }

    fn sleep(&self, thread_id: ThreadId, duration: Duration) -> SleepFuture<'_> {
        let outgoing = self.outgoing.clone();
        let thread_state_manager = self.thread_state_manager.clone();
        let pending_sleeps = Arc::clone(&self.pending_sleeps);
        Box::pin(async move {
            let outgoing = outgoing
                .upgrade()
                .context("app-server current-time provider is unavailable")?;
            let duration_ms = u64::try_from(duration.as_millis())
                .context("external sleep duration exceeds the supported range")?;
            let sleep_id = Uuid::now_v7().to_string();
            let key = SleepKey {
                thread_id: thread_id.to_string(),
                sleep_id: sleep_id.clone(),
            };
            // Register before notifying so an eager wake cannot race with the notification.
            let wake_rx = pending_sleeps.register(key.clone());
            let _guard = PendingSleepGuard {
                pending_sleeps,
                key,
            };
            notify_external_sleep(
                outgoing,
                thread_state_manager,
                thread_id,
                sleep_id,
                duration_ms,
            )
            .await?;
            wake_rx.await.context("external sleep was canceled")?;
            Ok(())
        })
    }
}

async fn request_current_time(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    thread_id: ThreadId,
) -> Result<DateTime<Utc>> {
    let deadline = Instant::now() + EXTERNAL_CLOCK_REQUEST_TIMEOUT;
    let connection_id =
        wait_for_current_time_connection(thread_state_manager, thread_id, deadline).await?;
    let connection_ids = [connection_id];
    let (request_id, rx) = outgoing
        .send_request_to_connections(
            Some(&connection_ids),
            ServerRequestPayload::CurrentTimeRead(CurrentTimeReadParams {
                thread_id: thread_id.to_string(),
            }),
            /*thread_id*/ None,
        )
        .await;

    let result = match timeout_at(deadline, rx).await {
        Ok(Ok(Ok(result))) => result,
        Ok(Ok(Err(err))) => {
            bail!(
                "current-time request failed: code={} message={}",
                err.code,
                err.message
            );
        }
        Ok(Err(err)) => bail!("current-time request was canceled: {err}"),
        Err(_) => {
            let _canceled = outgoing.cancel_request(&request_id).await;
            bail!(
                "current-time request timed out after {}s",
                EXTERNAL_CLOCK_REQUEST_TIMEOUT.as_secs()
            );
        }
    };
    let response: CurrentTimeReadResponse =
        serde_json::from_value(result).context("invalid current-time response")?;

    DateTime::from_timestamp(response.current_time_at, 0)
        .ok_or_else(|| anyhow!("current-time response is outside the supported range"))
}

async fn notify_external_sleep(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    thread_id: ThreadId,
    sleep_id: String,
    duration_ms: u64,
) -> Result<()> {
    let deadline = Instant::now() + EXTERNAL_CLOCK_REQUEST_TIMEOUT;
    let connection_id =
        wait_for_current_time_connection(thread_state_manager, thread_id, deadline).await?;
    outgoing
        .send_server_notification_to_connections(
            &[connection_id],
            ServerNotification::CurrentTimeSleep(CurrentTimeSleepNotification {
                thread_id: thread_id.to_string(),
                sleep_id,
                duration_ms,
            }),
        )
        .await;
    Ok(())
}

async fn wait_for_current_time_connection(
    thread_state_manager: ThreadStateManager,
    thread_id: ThreadId,
    deadline: Instant,
) -> Result<ConnectionId> {
    timeout_at(
        deadline,
        thread_state_manager.wait_for_thread_subscriber(thread_id),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "timed out waiting for a client to subscribe to the thread after {}s",
            EXTERNAL_CLOCK_REQUEST_TIMEOUT.as_secs()
        )
    })?;
    let connection_ids = thread_state_manager
        .subscribed_connection_ids(thread_id)
        .await;
    require_single_current_time_connection(&connection_ids)
}

fn require_single_current_time_connection(connection_ids: &[ConnectionId]) -> Result<ConnectionId> {
    // External clocks are not interchangeable, so do not choose one silently.
    match connection_ids {
        [connection_id] => Ok(*connection_id),
        _ => bail!(
            "expected exactly one client subscribed to the thread, found {}",
            connection_ids.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::require_single_current_time_connection;
    use crate::outgoing_message::ConnectionId;

    #[test]
    fn current_time_connection_must_be_unambiguous() {
        assert_eq!(
            require_single_current_time_connection(&[ConnectionId(7)]).unwrap(),
            ConnectionId(7)
        );
        assert_eq!(
            require_single_current_time_connection(&[])
                .unwrap_err()
                .to_string(),
            "expected exactly one client subscribed to the thread, found 0"
        );
        assert_eq!(
            require_single_current_time_connection(&[ConnectionId(7), ConnectionId(8)])
                .unwrap_err()
                .to_string(),
            "expected exactly one client subscribed to the thread, found 2"
        );
    }
}
