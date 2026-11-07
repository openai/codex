//! Converts supported cloud transcript events without interpreting executor paths locally.

use codex_app_server_protocol as protocol;
use codex_cloud_client as cloud;
use codex_protocol::models::ImageDetail;
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
    thread_id: String,
    update: cloud::TranscriptUpdate,
) -> Result<Option<protocol::ServerNotification>> {
    use protocol::ServerNotification as Notification;
    Ok(Some(match update {
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
        cloud::TranscriptUpdate::ItemStarted(item) => {
            let Some(content) = item_summary(item.item) else {
                return Ok(None);
            };
            Notification::ItemStarted(protocol::ItemStartedNotification::new(
                thread_id,
                item.turn_id,
                content.payload,
                item.at_ms,
            ))
        }
        cloud::TranscriptUpdate::ItemCompleted(item) => {
            let Some(content) = item_summary(item.item) else {
                return Ok(None);
            };
            Notification::ItemCompleted(protocol::ItemCompletedNotification::new(
                thread_id,
                item.turn_id,
                content.payload,
                /*started_at_ms*/ None,
                item.at_ms,
            ))
        }
        cloud::TranscriptUpdate::CommandOutputDelta(_)
        | cloud::TranscriptUpdate::FileChangeOutputDelta(_)
        | cloud::TranscriptUpdate::FileChangePatchUpdated(_) => return Ok(None),
    }))
}

pub(super) fn item_summary(item: cloud::ItemSummary) -> Option<protocol::ThreadItem> {
    let payload = match item {
        cloud::ItemSummary::UserMessage {
            id,
            client_id,
            content,
        } => protocol::ThreadItemPayload::UserMessage {
            id,
            client_id,
            content: content
                .into_iter()
                .map(|item| match item {
                    cloud::UserMessageInput::Text {
                        text,
                        text_elements,
                    } => protocol::UserInput::Text {
                        text,
                        text_elements: text_elements
                            .into_iter()
                            .map(|element| {
                                protocol::TextElement::new(
                                    protocol::ByteRange {
                                        start: element.start,
                                        end: element.end,
                                    },
                                    element.placeholder,
                                )
                            })
                            .collect(),
                    },
                    cloud::UserMessageInput::Image { image, detail } => {
                        protocol::UserInput::Image {
                            image: match image {
                                cloud::ImageReference::Url(url) => {
                                    protocol::ImageReference::Inline { url }
                                }
                                cloud::ImageReference::FileId(file_id) => {
                                    protocol::ImageReference::File { file_id }
                                }
                            },
                            detail: detail.map(|detail| match detail {
                                cloud::ImageDetail::Auto => ImageDetail::Auto,
                                cloud::ImageDetail::Low => ImageDetail::Low,
                                cloud::ImageDetail::High => ImageDetail::High,
                                cloud::ImageDetail::Original => ImageDetail::Original,
                            }),
                        }
                    }
                })
                .collect(),
        },
        cloud::ItemSummary::AgentMessage {
            id,
            text,
            questions,
        } => protocol::ThreadItemPayload::AgentMessage {
            moderation: None,
            id,
            text,
            phase: None,
            memory_citation: None,
            delivery: None,
            questions: questions.map(|questions| {
                questions
                    .into_iter()
                    .map(|question| protocol::AsyncUserInputQuestion {
                        title: question.title,
                        options: question.options,
                    })
                    .collect()
            }),
        },
        cloud::ItemSummary::Reasoning { .. }
        | cloud::ItemSummary::CommandExecution(_)
        | cloud::ItemSummary::FileChange { .. } => return None,
    };
    Some(protocol::ThreadItem::untimed(payload))
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
