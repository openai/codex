//! The lifetime of one continuous selection of an executor.

use std::sync::Arc;

use async_channel::Receiver;
use async_channel::Sender;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentConnectionState;
use codex_protocol::protocol::EnvironmentConnectionEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;
use tokio_util::task::AbortOnDropHandle;

/// Tracks one executor's membership in a thread's active or configured environment list.
/// Turn snapshots share the attachment to access the executor and observe when it leaves that list.
///
/// Removing the executor drops the selection's guard and cancels `detached`, even while
/// an older snapshot holds the attachment. Selecting the same executor again creates a new attachment.
pub(crate) struct EnvironmentAttachment {
    pub(crate) environment: Arc<Environment>,
    pub(crate) detached: CancellationToken,
    _connection_events_task: Option<AbortOnDropHandle<()>>,
}

impl EnvironmentAttachment {
    /// Returns the attachment, its detach guard, and a receiver for remote connection events.
    /// Snapshots share the attachment while the selection owns the guard, so dropping the guard
    /// signals detach immediately. The receiver starts at construction and can be dropped by
    /// callers that do not publish events. Local executors have no connection stream.
    pub(crate) fn new(
        environment_id: &str,
        environment: Arc<Environment>,
    ) -> (Arc<Self>, DropGuard, Option<Receiver<Event>>) {
        let detached = CancellationToken::new();
        let (connection_events_task, events) = environment
            .subscribe_connection_state()
            .map(|connection_state| {
                let (tx_event, events) = async_channel::unbounded();
                let task = tokio::spawn(forward_connection_events(
                    connection_state,
                    environment_id.to_owned(),
                    tx_event,
                    detached.clone(),
                ));
                (AbortOnDropHandle::new(task), events)
            })
            .unzip();
        let attachment = Arc::new(Self {
            environment,
            detached,
            _connection_events_task: connection_events_task,
        });
        let detach_on_drop = attachment.detached.clone().drop_guard();
        (attachment, detach_on_drop, events)
    }
}

async fn forward_connection_events(
    mut connection_state: watch::Receiver<EnvironmentConnectionState>,
    environment_id: String,
    tx_event: Sender<Event>,
    detached: CancellationToken,
) {
    loop {
        let state = tokio::select! {
            biased;
            _ = detached.cancelled() => return,
            _ = tx_event.closed() => return,
            changed = connection_state.changed() => {
                if changed.is_err() {
                    return;
                }
                *connection_state.borrow_and_update()
            }
        };
        let msg = match state {
            EnvironmentConnectionState::Recovering => continue,
            EnvironmentConnectionState::Connected => {
                EventMsg::EnvironmentConnected(EnvironmentConnectionEvent {
                    environment_id: environment_id.clone(),
                })
            }
            EnvironmentConnectionState::Disconnected => {
                EventMsg::EnvironmentDisconnected(EnvironmentConnectionEvent {
                    environment_id: environment_id.clone(),
                })
            }
        };
        if tx_event
            .try_send(Event {
                id: String::new(),
                msg,
            })
            .is_err()
        {
            return;
        }
    }
}
