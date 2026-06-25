use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::CurrentTimeReadParams;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::ServerRequestPayload;
use codex_core::SleepFuture;
use codex_core::TimeFuture;
use codex_core::TimeProvider;
use codex_protocol::ThreadId;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout_at;

use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::thread_state::ThreadStateManager;

const CURRENT_TIME_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn app_server_time_provider(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
) -> Arc<dyn TimeProvider> {
    Arc::new(AppServerTimeProvider {
        outgoing: Arc::downgrade(&outgoing),
        thread_state_manager,
    })
}

struct AppServerTimeProvider {
    outgoing: Weak<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
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

    fn sleep(&self, _thread_id: ThreadId, duration: Duration) -> SleepFuture<'_> {
        Box::pin(async move {
            tokio::time::sleep(duration).await;
            Ok(())
        })
    }
}

async fn request_current_time(
    outgoing: Arc<OutgoingMessageSender>,
    thread_state_manager: ThreadStateManager,
    thread_id: ThreadId,
) -> Result<DateTime<Utc>> {
    let deadline = Instant::now() + CURRENT_TIME_REQUEST_TIMEOUT;
    timeout_at(
        deadline,
        thread_state_manager.wait_for_thread_subscriber(thread_id),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "timed out waiting for a client to subscribe to the thread after {}s",
            CURRENT_TIME_REQUEST_TIMEOUT.as_secs()
        )
    })?;
    let connection_ids = thread_state_manager
        .subscribed_connection_ids(thread_id)
        .await;
    let connection_id = require_single_current_time_connection(&connection_ids)?;
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
                CURRENT_TIME_REQUEST_TIMEOUT.as_secs()
            );
        }
    };
    let response: CurrentTimeReadResponse =
        serde_json::from_value(result).context("invalid current-time response")?;

    DateTime::from_timestamp(response.current_time_at, 0)
        .ok_or_else(|| anyhow!("current-time response is outside the supported range"))
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
