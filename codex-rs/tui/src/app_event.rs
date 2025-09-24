use std::path::PathBuf;

use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::Event;
use codex_file_search::FileMatch;
use codex_protocol::mcp_protocol::ConversationId;

use crate::session_id::SessionId;

use crate::history_cell::HistoryCell;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    CodexEvent {
        session_id: SessionId,
        conversation_id: ConversationId,
        event: Event,
    },

    /// Start a new session.
    #[allow(dead_code)]
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

    InsertHistoryCell {
        session_id: SessionId,
        conversation_id: Option<ConversationId>,
        cell: Box<dyn HistoryCell>,
    },

    StartCommitAnimation,
    StopCommitAnimation,
    CommitTick,

    /// Begin creating a new conversation thread derived from the active session.
    StartThread,

    /// Start a brand-new blank thread while keeping existing ones alive.
    NewBlankThread,

    /// Clear the currently active thread back to an empty context.
    ClearActiveThread,

    /// Prompt the user to close the active thread.
    PromptCloseActiveThread,

    /// Provide the latest user input so the app can derive a thread name.
    SuggestThreadName {
        session_id: SessionId,
        text: String,
    },

    /// Toggle thread picker showing active conversations.
    ToggleThreadPicker,

    /// Switch to the specified thread index.
    SwitchThread(usize),

    /// User invoked `/quit`; app decides whether to exit or close thread.
    QuitRequested,

    /// Close a specific thread, optionally preparing a summary for the parent.
    CloseThread {
        index: usize,
        summarize: bool,
    },

    /// Update the current reasoning effort in the running app and widget.
    UpdateReasoningEffort(Option<ReasoningEffort>),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Persist the selected model and reasoning effort to the appropriate config.
    PersistModelSelection {
        model: String,
        effort: Option<ReasoningEffort>,
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
}
