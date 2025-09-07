use codex_core::protocol::ConversationHistoryResponseEvent;
use codex_core::protocol::Event;
use codex_file_search::FileMatch;

use crate::history_cell::HistoryCell;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;

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

    InsertHistoryCell(Box<dyn HistoryCell>),

    StartCommitAnimation,
    StopCommitAnimation,
    CommitTick,

    /// Update the current reasoning effort in the running app and widget.
    UpdateReasoningEffort(ReasoningEffort),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Update the current approval policy in the running app and widget.
    UpdateAskForApprovalPolicy(AskForApproval),

    /// Update the current sandbox policy in the running app and widget.
    UpdateSandboxPolicy(SandboxPolicy),

    /// Forwarded conversation history snapshot from the current conversation.
    ConversationHistory(ConversationHistoryResponseEvent),

    /// Toggle post-turn judge on/off.
    UpdateTurnJudgeEnabled(bool),
    /// Update judge prompt (None → default).
    UpdateTurnJudgePrompt(Option<String>),
    /// Toggle autopilot (auto-continue when judge approves).
    UpdateAutopilotEnabled(bool),

    /// Toggle Yes‑Man mode: always continue without running judge.
    UpdateYesManEnabled(bool),

    /// Open the resume picker UI.
    OpenResumePicker,

    /// Open the most recent Judge session for this workspace.
    OpenLastJudgeSession,

    /// Resume from a specific rollout path.
    ResumeFromPath(std::path::PathBuf),

    /// Toggle Reviewer mode (PRD-aware specialist).
    UpdateReviewerEnabled(bool),

    /// Open the most recent Reviewer session for this workspace.
    OpenLastReviewerSession,

    /// Update the model used by the Reviewer session.
    UpdateReviewerModel(String),

    /// Update the reasoning effort used by the Reviewer session.
    UpdateReviewerEffort(ReasoningEffort),

    /// Open the Autopilot & Review popup (rebuild in-place).
    OpenAutopilotPopup,

    /// Toggle PatchGate integration (TUI-level; record intent and drive Reviewer integration).
    UpdatePatchGateEnabled(bool),
    /// Toggle PatchGate "permissive" mode (record-only; informational badge/logging).
    UpdatePatchGatePermissive(bool),

    /// Open the Reviewer Model selection sub-menu.
    OpenReviewerModelPopup,

    /// Show a simple text overlay with a title.
    ShowTextOverlay {
        title: String,
        text: String,
    },
}
