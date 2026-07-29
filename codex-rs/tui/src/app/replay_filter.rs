//! Helpers for deciding which buffered events to replay when switching threads.

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;

use super::ThreadBufferedEvent;
use super::ThreadEventSnapshot;

pub(super) fn snapshot_has_pending_interactive_request(snapshot: &ThreadEventSnapshot) -> bool {
    snapshot.events.iter().any(|event| {
        matches!(
            event,
            ThreadBufferedEvent::Request(request)
                if matches!(
                    request.as_ref(),
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
        ThreadBufferedEvent::Notification(notification)
            if matches!(
                notification.as_ref(),
                ServerNotification::Warning(_)
                    | ServerNotification::GuardianWarning(_)
                    | ServerNotification::ConfigWarning(_)
            )
    )
}
