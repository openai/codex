//! Typed live transcript updates for the attached thread.
use crate::Error;
use crate::transcript_wire as wire;
use std::fmt;

/// A live update, not a history snapshot or a replay guarantee.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TranscriptEvent {
    pub thread_id: String,
    pub emitted_at_ms: Option<i64>,
    pub update: TranscriptUpdate,
}

#[derive(Clone, PartialEq, Eq)]
pub enum TranscriptUpdate {
    AgentMessageDelta(ItemDelta),
    CommandOutputDelta(ItemDelta),
    FileChangeOutputDelta(ItemDelta),
    FileChangePatchUpdated(PatchUpdate),
    TurnStarted(TurnSummary),
    TurnCompleted(TurnSummary),
}
#[derive(Clone, PartialEq, Eq)]
pub struct ItemDelta {
    pub item_id: String,
    pub turn_id: String,
    pub delta: String,
}
#[derive(Clone, PartialEq, Eq)]
pub struct PatchUpdate {
    pub item_id: String,
    pub turn_id: String,
    pub changes: Vec<FileUpdate>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct FileUpdate {
    pub path: String,
    pub diff: String,
    pub kind: PatchKind,
}
#[derive(Clone, PartialEq, Eq)]
pub enum PatchKind {
    Add,
    Delete,
    Update {
        move_path: Option<String>,
    },
    /// The server reported an unrecognized change kind.
    Unknown,
}
/// Status, timing, and failure details for a turn.
#[derive(Clone, PartialEq, Eq)]
pub struct TurnSummary {
    pub id: String,
    pub status: TurnStatus,
    /// Turn start time in Unix seconds, if reported by the server.
    pub started_at: Option<i64>,
    /// Turn completion time in Unix seconds, if reported by the server.
    pub completed_at: Option<i64>,
    /// Turn duration in milliseconds, if reported by the server.
    pub duration_ms: Option<i64>,
    pub error: Option<TurnFailure>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
    Unknown(i32),
}
#[derive(Clone, PartialEq, Eq)]
pub struct TurnFailure {
    pub message: String,
    pub additional_details: Option<String>,
}
// Each public payload type is safe to format independently, not just through Event.
macro_rules! redacted_debug {
    ($($ty:ty),* $(,)?) => {$(impl fmt::Debug for $ty {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(concat!(stringify!($ty), "([redacted])"))
        }
    })*};
}
redacted_debug!(
    TranscriptEvent,
    TranscriptUpdate,
    ItemDelta,
    PatchUpdate,
    FileUpdate,
    PatchKind,
    TurnSummary,
    TurnFailure
);

// Field numbers in the ASB Notification.value oneof.
const TURN_STARTED: u32 = 21;
const TURN_COMPLETED: u32 = 23;
const ITEM_AGENT_MESSAGE_DELTA: u32 = 32;
const ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: u32 = 37;
const ITEM_FILE_CHANGE_OUTPUT_DELTA: u32 = 39;
const ITEM_FILE_CHANGE_PATCH_UPDATED: u32 = 40;

enum DecodeFailure {
    MissingField,
    InvalidResponse,
}

impl From<prost::DecodeError> for DecodeFailure {
    fn from(_: prost::DecodeError) -> Self {
        Self::InvalidResponse
    }
}

fn required<T>(value: Option<T>) -> Result<T, DecodeFailure> {
    value.ok_or(DecodeFailure::MissingField)
}
impl TranscriptEvent {
    pub(crate) fn decode(thread_id: &str, payload: &[u8]) -> Result<Option<Self>, Error> {
        match Self::decode_present_fields(thread_id, payload) {
            Ok(event) => Ok(event),
            Err(DecodeFailure::MissingField) => Ok(None),
            Err(DecodeFailure::InvalidResponse) => Err(Error::InvalidResponse),
        }
    }

    fn decode_present_fields(
        thread_id: &str,
        payload: &[u8],
    ) -> Result<Option<Self>, DecodeFailure> {
        let Some((tag, values)) = wire::notification(payload)? else {
            return Ok(None);
        };
        let (owner, emitted_at_ms, update) = match tag {
            ITEM_AGENT_MESSAGE_DELTA
            | ITEM_COMMAND_EXECUTION_OUTPUT_DELTA
            | ITEM_FILE_CHANGE_OUTPUT_DELTA => {
                let envelope = wire::merge_messages::<wire::Envelope<wire::Delta>>(values)?;
                let params = required(envelope.params)?;
                let delta = ItemDelta {
                    item_id: required(params.item_id)?,
                    turn_id: required(params.turn_id)?,
                    delta: required(params.delta)?,
                };
                let update = match tag {
                    ITEM_AGENT_MESSAGE_DELTA => TranscriptUpdate::AgentMessageDelta(delta),
                    ITEM_COMMAND_EXECUTION_OUTPUT_DELTA => {
                        TranscriptUpdate::CommandOutputDelta(delta)
                    }
                    ITEM_FILE_CHANGE_OUTPUT_DELTA => TranscriptUpdate::FileChangeOutputDelta(delta),
                    _ => return Ok(None),
                };
                (required(params.thread_id)?, envelope.emitted_at_ms, update)
            }
            ITEM_FILE_CHANGE_PATCH_UPDATED => {
                let envelope = wire::merge_messages::<wire::Envelope<wire::PatchUpdate>>(values)?;
                let params = required(envelope.params)?;
                let changes = required(params.changes)?
                    .values
                    .into_iter()
                    .map(|change| {
                        let kind = match required(change.kind)?.value {
                            Some(wire::Kind::Add(_)) => PatchKind::Add,
                            Some(wire::Kind::Delete(_)) => PatchKind::Delete,
                            Some(wire::Kind::Update(value)) => PatchKind::Update {
                                move_path: value.move_path,
                            },
                            None => PatchKind::Unknown,
                        };
                        Ok(FileUpdate {
                            path: required(change.path)?,
                            diff: required(change.diff)?,
                            kind,
                        })
                    })
                    .collect::<Result<_, DecodeFailure>>()?;
                let update = PatchUpdate {
                    item_id: required(params.item_id)?,
                    turn_id: required(params.turn_id)?,
                    changes,
                };
                (
                    required(params.thread_id)?,
                    envelope.emitted_at_ms,
                    TranscriptUpdate::FileChangePatchUpdated(update),
                )
            }
            TURN_STARTED | TURN_COMPLETED => {
                let envelope = wire::merge_messages::<wire::Envelope<wire::TurnUpdate>>(values)?;
                let params = required(envelope.params)?;
                let turn = required(params.turn)?;
                let status = match required(turn.status)? {
                    1 => TurnStatus::Completed,
                    2 => TurnStatus::Interrupted,
                    3 => TurnStatus::Failed,
                    4 => TurnStatus::InProgress,
                    value => TurnStatus::Unknown(value),
                };
                let error = turn
                    .error
                    .map(|error| {
                        Ok::<_, DecodeFailure>(TurnFailure {
                            message: required(error.message)?,
                            additional_details: error.additional_details,
                        })
                    })
                    .transpose()?;
                let summary = TurnSummary {
                    id: required(turn.id)?,
                    status,
                    started_at: turn.started_at,
                    completed_at: turn.completed_at,
                    duration_ms: turn.duration_ms,
                    error,
                };
                let update = if tag == TURN_STARTED {
                    TranscriptUpdate::TurnStarted(summary)
                } else {
                    TranscriptUpdate::TurnCompleted(summary)
                };
                (required(params.thread_id)?, envelope.emitted_at_ms, update)
            }
            _ => return Ok(None),
        };
        if owner != thread_id {
            // Attach also carries account-wide lifecycle notifications. Preserve them
            // as opaque events rather than ending this thread's transcript stream.
            return if matches!(tag, TURN_STARTED | TURN_COMPLETED) {
                Ok(None)
            } else {
                Err(DecodeFailure::InvalidResponse)
            };
        }
        Ok(Some(Self {
            thread_id: owner,
            emitted_at_ms,
            update,
        }))
    }
}
