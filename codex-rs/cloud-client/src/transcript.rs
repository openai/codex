//! Typed live transcript updates for the attached thread.
use crate::Error;
use crate::Event;
use crate::ThreadStatus;
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
    ItemStarted(crate::ItemLifecycle),
    ItemCompleted(crate::ItemLifecycle),
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
const THREAD_STATUS_CHANGED: u32 = 3;
const TURN_STARTED: u32 = 21;
const TURN_COMPLETED: u32 = 23;
pub(super) const ITEM_STARTED: u32 = 27;
const ITEM_COMPLETED: u32 = 31;
const ITEM_AGENT_MESSAGE_DELTA: u32 = 32;
const ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: u32 = 37;
const ITEM_FILE_CHANGE_OUTPUT_DELTA: u32 = 39;
const ITEM_FILE_CHANGE_PATCH_UPDATED: u32 = 40;

pub(super) enum DecodeFailure {
    MissingField,
    InvalidResponse,
}

impl From<prost::DecodeError> for DecodeFailure {
    fn from(_: prost::DecodeError) -> Self {
        Self::InvalidResponse
    }
}

pub(super) fn required<T>(value: Option<T>) -> Result<T, DecodeFailure> {
    value.ok_or(DecodeFailure::MissingField)
}
impl Event {
    /// Returns a supported status update for this thread from an Attach notification.
    pub fn thread_status_for(&self, thread_id: &str) -> Option<ThreadStatus> {
        let Self::Notification(bytes) = self else {
            return None;
        };
        let (tag, values) = wire::notification(bytes).ok().flatten()?;
        if tag != THREAD_STATUS_CHANGED {
            return None;
        }
        let envelope = wire::merge_messages::<wire::Envelope<wire::StatusUpdate>>(values).ok()?;
        let params = envelope.params?;
        if params.thread_id.as_deref() != Some(thread_id) {
            return None;
        }
        crate::activation::parse_status(params.status?).ok()
    }
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
            ITEM_STARTED | ITEM_COMPLETED => match crate::items::decode(tag, values)? {
                Some(decoded) => decoded,
                None => return Ok(None),
            },
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
                    .map(FileUpdate::try_from)
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
                let summary = TurnSummary::try_from(required(params.turn)?)?;
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

impl TryFrom<wire::Change> for FileUpdate {
    type Error = DecodeFailure;
    fn try_from(change: wire::Change) -> Result<Self, DecodeFailure> {
        let kind = match required(change.kind)?.value {
            Some(wire::Kind::Add(_)) => PatchKind::Add,
            Some(wire::Kind::Delete(_)) => PatchKind::Delete,
            Some(wire::Kind::Update(value)) => PatchKind::Update {
                move_path: value.move_path,
            },
            None => PatchKind::Unknown,
        };
        Ok(Self {
            path: required(change.path)?,
            diff: required(change.diff)?,
            kind,
        })
    }
}

impl TryFrom<wire::Turn> for TurnSummary {
    type Error = DecodeFailure;

    fn try_from(turn: wire::Turn) -> Result<Self, DecodeFailure> {
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
        Ok(Self {
            id: required(turn.id)?,
            status,
            started_at: turn.started_at,
            completed_at: turn.completed_at,
            duration_ms: turn.duration_ms,
            error,
        })
    }
}
