//! Converts supported cloud transcript events without interpreting executor paths locally.

use codex_app_server_protocol as protocol;
use codex_cloud_client as cloud;
use color_eyre::eyre::Result;
use color_eyre::eyre::bail;

pub(super) fn turn(summary: cloud::TurnSummary) -> Result<protocol::Turn> {
    Ok(protocol::Turn {
        id: summary.id,
        root_turn_id: None,
        items: Vec::new(),
        items_view: protocol::TurnItemsView::NotLoaded,
        status: match summary.status {
            cloud::TurnStatus::Completed => protocol::TurnStatus::Completed,
            cloud::TurnStatus::Interrupted => protocol::TurnStatus::Interrupted,
            cloud::TurnStatus::Failed => protocol::TurnStatus::Failed,
            cloud::TurnStatus::InProgress => protocol::TurnStatus::InProgress,
            cloud::TurnStatus::Unknown(_) => {
                bail!("The cloud returned an unsupported turn status.");
            }
        },
        error: summary.error.map(|error| protocol::TurnError {
            message: error.message,
            additional_details: error.additional_details,
            codex_error_info: None,
            misalignment: None,
        }),
        started_at: summary.started_at,
        completed_at: summary.completed_at,
        duration_ms: summary.duration_ms,
    })
}

pub(super) fn transcript(
    event: cloud::TranscriptEvent,
) -> Result<Option<protocol::ServerNotification>> {
    use protocol::ServerNotification as Notification;
    let thread_id = event.thread_id;
    Ok(Some(match event.update {
        cloud::TranscriptUpdate::TurnStarted(summary) => {
            Notification::TurnStarted(protocol::TurnStartedNotification {
                thread_id,
                turn: turn(summary)?,
            })
        }
        cloud::TranscriptUpdate::TurnCompleted(summary) => {
            Notification::TurnCompleted(protocol::TurnCompletedNotification {
                thread_id,
                turn: turn(summary)?,
            })
        }
        cloud::TranscriptUpdate::AgentMessageDelta(delta) => {
            Notification::AgentMessageDelta(protocol::AgentMessageDeltaNotification {
                thread_id,
                turn_id: delta.turn_id,
                item_id: delta.item_id,
                delta: delta.delta,
            })
        }
        cloud::TranscriptUpdate::ItemStarted(_)
        | cloud::TranscriptUpdate::ItemCompleted(_)
        | cloud::TranscriptUpdate::CommandOutputDelta(_)
        | cloud::TranscriptUpdate::FileChangeOutputDelta(_)
        | cloud::TranscriptUpdate::FileChangePatchUpdated(_) => return Ok(None),
    }))
}
