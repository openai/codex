//! Helpers for deciding which buffered events to replay when switching threads.

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;

use super::ThreadBufferedEvent;
use super::ThreadEventSnapshot;

pub(super) fn snapshot_has_pending_interactive_request(snapshot: &ThreadEventSnapshot) -> bool {
    snapshot.events.iter().any(|event| {
        matches!(
            event,
            ThreadBufferedEvent::Request(
                ServerRequest::CommandExecutionRequestApproval { .. }
                    | ServerRequest::FileChangeRequestApproval { .. }
                    | ServerRequest::McpServerElicitationRequest { .. }
                    | ServerRequest::PermissionsRequestApproval { .. }
                    | ServerRequest::ToolRequestUserInput { .. }
            )
        )
    })
}

pub(super) fn event_is_notice(event: &ThreadBufferedEvent) -> bool {
    matches!(
        event,
        ThreadBufferedEvent::Notification(
            ServerNotification::Warning(_) | ServerNotification::ConfigWarning(_)
        )
    )
}
