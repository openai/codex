//! Selected item summaries for live start/completion events, without interpreting executor paths.
use crate::FileUpdate;
use crate::ImageDetail;
use crate::ImageReference;
use crate::TranscriptUpdate;
use crate::UserMessageInput;
use crate::UserMessageTextElement;
use crate::item_wire as wire;
use crate::transcript::DecodeFailure;
use crate::transcript::ITEM_STARTED;
use crate::transcript::required;
use crate::transcript_wire::Envelope;
use crate::transcript_wire::SelectedMessages;
use crate::transcript_wire::merge_messages;
use std::fmt;

/// Lifecycle identity and timestamp for a typed item event.
#[derive(Clone, PartialEq, Eq)]
pub struct ItemLifecycle {
    pub turn_id: String,
    /// Start or completion time in Unix milliseconds, according to the event variant.
    pub at_ms: i64,
    pub item: ItemSummary,
}
/// Selected fields only; this is not a complete display model.
#[derive(Clone, PartialEq, Eq)]
pub enum ItemSummary {
    UserMessage {
        id: String,
        client_id: Option<String>,
        content: Vec<UserMessageInput>,
    },
    AgentMessage {
        id: String,
        text: String,
        questions: Option<Vec<AgentQuestion>>,
    },
    Reasoning {
        id: String,
        summary: Vec<String>,
        content: Vec<String>,
    },
    CommandExecution(CommandSummary),
    FileChange {
        id: String,
        changes: Vec<FileUpdate>,
        status: ItemStatus,
    },
}
/// A structured question attached to an agent message.
#[derive(Clone, PartialEq, Eq)]
pub struct AgentQuestion {
    pub title: String,
    pub options: Option<Vec<String>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct CommandSummary {
    pub id: String,
    pub command: String,
    /// Executor-native path. Do not interpret it as a path on the client machine.
    pub cwd: String,
    pub actions: Vec<CommandAction>,
    pub source: CommandExecutionSource,
    pub aggregated_output: Option<String>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i64>,
    pub status: ItemStatus,
}
#[derive(Clone, PartialEq, Eq)]
pub enum CommandAction {
    Read {
        command: String,
        name: String,
        path: String,
    },
    ListFiles {
        command: String,
        path: Option<String>,
    },
    Search {
        command: String,
        query: Option<String>,
        path: Option<String>,
    },
    Unknown {
        command: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandExecutionSource {
    Agent,
    UserShell,
    UnifiedExecStartup,
    UnifiedExecInteraction,
}
/// CommandExecutionStatus and PatchApplyStatus share these wire values.
/// Completion of the item lifecycle does not imply the command or patch succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
    Unknown(i32),
}
impl From<i32> for ItemStatus {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::InProgress,
            2 => Self::Completed,
            3 => Self::Failed,
            4 => Self::Declined,
            value => Self::Unknown(value),
        }
    }
}
macro_rules! redacted_debug {
    ($($ty:ty),*) => {$(impl fmt::Debug for $ty {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(concat!(stringify!($ty), "([redacted])"))
        }
    })*};
}
redacted_debug!(
    ItemLifecycle,
    ItemSummary,
    AgentQuestion,
    CommandSummary,
    CommandAction
);

pub(crate) fn decode(
    tag: u32,
    values: SelectedMessages<'_>,
) -> Result<Option<(String, Option<i64>, TranscriptUpdate)>, DecodeFailure> {
    let (thread_id, turn_id, at_ms, item, emitted_at_ms) = if tag == ITEM_STARTED {
        let envelope = merge_messages::<Envelope<wire::Started>>(values)?;
        let params = required(envelope.params)?;
        (
            params.thread_id,
            params.turn_id,
            params.at_ms,
            params.item,
            envelope.emitted_at_ms,
        )
    } else {
        let envelope = merge_messages::<Envelope<wire::Completed>>(values)?;
        let params = required(envelope.params)?;
        (
            params.thread_id,
            params.turn_id,
            params.at_ms,
            params.item,
            envelope.emitted_at_ms,
        )
    };
    let Some(item) = summary(required(item)?)? else {
        return Ok(None);
    };
    let lifecycle = ItemLifecycle {
        turn_id: required(turn_id)?,
        at_ms: required(at_ms)?,
        item,
    };
    let update = if tag == ITEM_STARTED {
        TranscriptUpdate::ItemStarted(lifecycle)
    } else {
        TranscriptUpdate::ItemCompleted(lifecycle)
    };
    Ok(Some((required(thread_id)?, emitted_at_ms, update)))
}

pub(crate) fn summary(item: wire::Item) -> Result<Option<ItemSummary>, DecodeFailure> {
    let summary = match item.value {
        Some(wire::Value::UserMessage(value)) => {
            let mut content = Vec::new();
            for input in required(value.content)?.values {
                let input = match input.value {
                    Some(crate::turns_wire::Value::Text(input)) => {
                        let text = required(input.text)?;
                        let text_elements = input
                            .text_elements
                            .map_or_else(Vec::new, |elements| elements.values)
                            .into_iter()
                            .map(|element| {
                                let range = required(element.byte_range)?;
                                let start = usize::try_from(required(range.start)?)
                                    .map_err(|_| DecodeFailure::MissingField)?;
                                let end = usize::try_from(required(range.end)?)
                                    .map_err(|_| DecodeFailure::MissingField)?;
                                text.get(start..end).ok_or(DecodeFailure::MissingField)?;
                                Ok(UserMessageTextElement {
                                    start,
                                    end,
                                    placeholder: element.placeholder,
                                })
                            })
                            .collect::<Result<_, DecodeFailure>>()?;
                        UserMessageInput::Text {
                            text,
                            text_elements,
                        }
                    }
                    Some(crate::turns_wire::Value::Image(input)) => {
                        let (image, detail) = match input.value {
                            Some(crate::turns_wire::ImageValue::Url(image)) => {
                                (ImageReference::Url(required(image.value)?), image.detail)
                            }
                            Some(crate::turns_wire::ImageValue::FileId(image)) => {
                                (ImageReference::FileId(required(image.value)?), image.detail)
                            }
                            None => return Ok(None),
                        };
                        let detail = match detail {
                            Some(1) => Some(ImageDetail::Auto),
                            Some(2) => Some(ImageDetail::Low),
                            Some(3) => Some(ImageDetail::High),
                            Some(4) => Some(ImageDetail::Original),
                            // Detail is display metadata. Keep the receipt identity if it is newer.
                            None | Some(_) => None,
                        };
                        UserMessageInput::Image { image, detail }
                    }
                    _ => return Ok(None),
                };
                content.push(input);
            }
            ItemSummary::UserMessage {
                id: required(value.id)?,
                client_id: value.client_id,
                content,
            }
        }
        Some(wire::Value::AgentMessage(value)) => ItemSummary::AgentMessage {
            id: required(value.id)?,
            text: required(value.text)?,
            questions: value
                .questions
                .map(|questions| {
                    questions
                        .values
                        .into_iter()
                        .map(|question| {
                            Ok(AgentQuestion {
                                title: required(question.title)?,
                                options: question.options.map(|options| options.values),
                            })
                        })
                        .collect::<Result<_, DecodeFailure>>()
                })
                .transpose()?,
        },
        Some(wire::Value::Reasoning(value)) => ItemSummary::Reasoning {
            id: required(value.id)?,
            summary: value.summary.map_or_else(Vec::new, |value| value.values),
            content: value.content.map_or_else(Vec::new, |value| value.values),
        },
        Some(wire::Value::Command(value)) => ItemSummary::CommandExecution(CommandSummary {
            id: required(value.id)?,
            command: required(value.command)?,
            cwd: required(value.cwd)?,
            actions: required(value.actions)?
                .values
                .into_iter()
                .map(command_action)
                .collect::<Result<_, _>>()?,
            status: required(value.status)?.into(),
            source: match value.source {
                None | Some(1) => CommandExecutionSource::Agent,
                Some(2) => CommandExecutionSource::UserShell,
                Some(3) => CommandExecutionSource::UnifiedExecStartup,
                Some(4) => CommandExecutionSource::UnifiedExecInteraction,
                Some(_) => return Ok(None),
            },
            aggregated_output: value.output,
            duration_ms: value.duration_ms,
            exit_code: value.exit_code,
        }),
        Some(wire::Value::FileChange(value)) => ItemSummary::FileChange {
            id: required(value.id)?,
            changes: required(value.changes)?
                .values
                .into_iter()
                .map(FileUpdate::try_from)
                .collect::<Result<_, _>>()?,
            status: required(value.status)?.into(),
        },
        _ => return Ok(None),
    };
    Ok(Some(summary))
}

fn command_action(action: wire::CommandAction) -> Result<CommandAction, DecodeFailure> {
    Ok(match required(action.value)? {
        wire::command_action::Value::Read(value) => CommandAction::Read {
            command: required(value.command)?,
            name: required(value.name)?,
            path: required(value.path)?,
        },
        wire::command_action::Value::ListFiles(value) => CommandAction::ListFiles {
            command: required(value.command)?,
            path: value.path,
        },
        wire::command_action::Value::Search(value) => CommandAction::Search {
            command: required(value.command)?,
            query: value.query,
            path: value.path,
        },
        wire::command_action::Value::Unknown(value) => CommandAction::Unknown {
            command: required(value.command)?,
        },
    })
}
