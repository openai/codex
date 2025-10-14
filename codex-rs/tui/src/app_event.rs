use std::collections::HashMap;
use std::path::PathBuf;

use codex_common::model_presets::ModelPreset;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::Event;
use codex_file_search::FileMatch;

use crate::bottom_pane::ApprovalRequest;
use crate::history_cell::HistoryCell;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;

#[derive(Debug, Clone, Copy)]
pub(crate) enum CheckpointAction {
    Save,
    Load,
}

#[derive(Debug, Clone)]
pub(crate) enum TodoAction {
    Add { text: String },
    List,
    Complete { index: usize },
    Auto { enabled: bool },
}

#[derive(Debug, Clone)]
pub(crate) enum AliasAction {
    Add { name: String },
    Store { name: String, prompt: String },
    Remove { name: String },
    List,
}

#[derive(Debug, Clone)]
pub(crate) enum CommitAction {
    Perform { message: Option<String>, auto: bool },
    SetAuto { enabled: bool },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    CodexEvent(Event),

    /// Start a new session.
    NewSession,

    /// Request to exit the application gracefully.
    ExitRequest,

    /// Forward an `Op` to the Agent. Using an `AppEvent` for this avoids
    /// bubbling channels through layers of widgets.
    CodexOp(codex_core::protocol::Op),

    /// Kick off an asynchronous file search for the given query (text after
    /// the `@`). Previous searches may be cancelled by the app layer so there
    /// is at most one in-flight search.
    StartFileSearch(String),

    /// Result of a completed asynchronous file search. The `query` echoes the
    /// original search term so the UI can decide whether the results are
    /// still relevant.
    FileSearchResult {
        query: String,
        matches: Vec<FileMatch>,
    },

    /// Result of computing a `/diff` command.
    DiffResult(String),
    /// Handle a `/checkpoint` command issued from the composer.
    CheckpointCommand {
        action: CheckpointAction,
        name: Option<String>,
    },
    /// Handle a `/todo` command issued from the composer.
    TodoCommand {
        action: TodoAction,
    },
    /// Handle a `/alias` command issued from the composer.
    AliasCommand {
        action: AliasAction,
    },
    /// Handle a `/commit` command issued from the composer.
    CommitCommand {
        action: CommitAction,
    },
    /// Enable or disable automatic checkpoints.
    SetCheckpointAutomation {
        enabled: bool,
    },
    /// Fired when an automatic checkpoint should be captured after a turn.
    AutoCheckpointTick,
    /// Fired when auto-commit should stage and commit changes after a turn.
    AutoCommitTick,

    InsertHistoryCell(Box<dyn HistoryCell>),

    StartCommitAnimation,
    StopCommitAnimation,
    CommitTick,

    /// Update the current reasoning effort in the running app and widget.
    UpdateReasoningEffort(Option<ReasoningEffort>),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Persist the selected model and reasoning effort to the appropriate config.
    PersistModelSelection {
        model: String,
        effort: Option<ReasoningEffort>,
    },

    /// Persist the global prompt that should auto-prepend to new sessions.
    PersistGlobalPrompt {
        prompt: Option<String>,
    },

    /// Persist the alarm script that should run after each completed turn.
    PersistAlarmScript {
        script: Option<String>,
    },

    /// Persist the global alias list so prompt shortcuts survive restarts.
    PersistAliases {
        aliases: HashMap<String, String>,
    },

    /// Open the reasoning selection popup after picking a model.
    OpenReasoningPopup {
        model: String,
        presets: Vec<ModelPreset>,
    },

    /// Update the current approval policy in the running app and widget.
    UpdateAskForApprovalPolicy(AskForApproval),

    /// Update the current sandbox policy in the running app and widget.
    UpdateSandboxPolicy(SandboxPolicy),

    /// Forwarded conversation history snapshot from the current conversation.
    ConversationHistory(ConversationPathResponseEvent),

    /// Open the branch picker option from the review popup.
    OpenReviewBranchPicker(PathBuf),

    /// Open the commit picker option from the review popup.
    OpenReviewCommitPicker(PathBuf),

    /// Open the custom prompt option from the review popup.
    OpenReviewCustomPrompt,

    /// Open the approval popup.
    FullScreenApprovalRequest(ApprovalRequest),
}
