//! Delivers configuration and background task warnings after a client initializes.
//! The runtime event loop owns the receiver and consumes background warnings once.

use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::WarningNotification;

use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;

pub(crate) struct StartupWarnings {
    pub(crate) config: Vec<ConfigWarningNotification>,
}

impl StartupWarnings {
    pub(crate) async fn send_config_to_connections(
        &self,
        outgoing: &OutgoingMessageSender,
        connection_ids: &[ConnectionId],
    ) {
        for notification in self.config.iter().cloned() {
            outgoing
                .send_server_notification_to_connections(
                    connection_ids,
                    ServerNotification::ConfigWarning(notification),
                )
                .await;
        }
    }
}

pub(crate) async fn send_warning(outgoing: &OutgoingMessageSender, message: String) {
    outgoing
        .send_server_notification(ServerNotification::Warning(WarningNotification {
            thread_id: None,
            message,
        }))
        .await;
}
