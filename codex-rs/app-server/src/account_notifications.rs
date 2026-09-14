//! Publishes account snapshots only while their authentication owner is current.
//! Superseded login attempts still receive a terminal completion notification.

use super::*;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_login::AuthChangeState;
use tokio::sync::watch;

pub(crate) enum AccountNotification {
    LoginCompleted(AccountLoginCompletedNotification),
    Updated(AccountUpdatedNotification),
}

impl OutgoingMessageSender {
    pub(crate) async fn send_account_notification(
        &self,
        connection_id: Option<ConnectionId>,
        auth_changes: &watch::Receiver<AuthChangeState>,
        owner_generation: u64,
        notification: AccountNotification,
    ) {
        let Ok(permit) = self.sender.reserve().await else {
            return;
        };
        // Keep the revision read lock through enqueue; never wait while holding it.
        let current_auth = auth_changes.borrow();
        let notification = match notification {
            AccountNotification::LoginCompleted(mut payload) => {
                if payload.success && current_auth.owner_generation != owner_generation {
                    payload.success = false;
                    payload.error = Some("account changed before sign-in completed".into());
                }
                ServerNotification::AccountLoginCompleted(payload)
            }
            AccountNotification::Updated(payload) => {
                if current_auth.owner_generation != owner_generation {
                    return;
                }
                ServerNotification::AccountUpdated(payload)
            }
        };
        let message = timestamped_server_notification(notification);
        permit.send(match connection_id {
            Some(connection_id) => OutgoingEnvelope::ToConnection {
                connection_id,
                message,
                write_complete_tx: None,
            },
            None => OutgoingEnvelope::Broadcast { message },
        });
    }
}

#[cfg(test)]
#[path = "account_notifications_tests.rs"]
mod tests;
