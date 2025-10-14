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
    UpdateReasoningEffort(Option<ReasoningEffort>),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Persist the selected model and reasoning effort to the appropriate config.
    PersistModelSelection {
        model: String,
        effort: Option<ReasoningEffort>,
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
    /// Resume the conversation from the backup rollout (\*.bak).
    RestoreContextFromBackup,
    /// On shutdown, back up and rewrite the current rollout file according to the
    /// current prune mask so the next resume starts from a pruned file.
    FinalizePruneOnShutdown,

    /// Open the branch picker option from the review popup.
    OpenReviewBranchPicker(PathBuf),

    /// Open the commit picker option from the review popup.
    OpenReviewCommitPicker(PathBuf),

    /// Open the custom prompt option from the review popup.
    OpenReviewCustomPrompt,

    // --- Advanced Prune (experimental) ---
    /// Open the non-destructive advanced prune view.
    OpenPruneAdvanced,
    /// Open the manual prune submenu (by category).
    OpenPruneManual,
    /// Confirm the currently selected manual prune category.
    ConfirmManualChanges,
    /// Apply the pending manual prune plan.
    ApplyManualPrune,
    /// Open the root prune menu.
    OpenPruneRoot,
    /// Open a confirmation dialog for manual prune of a specific category.
    OpenPruneManualConfirm {
        category: codex_core::protocol::PruneCategory,
        label: String,
    },
    /// Close the advanced prune view.
    PruneAdvancedClosed,
    /// Root prune menu dismissed (via Esc or completion).
    PruneRootClosed,
    /// Toggle keep marker for an index in the advanced list.
    ToggleKeepIndex {
        idx: usize,
    },
    /// Toggle delete marker for an index in the advanced list.
    ToggleDeleteIndex {
        idx: usize,
    },
    /// Apply staged inclusion/deletion changes.
    ConfirmAdvancedChanges,
    /// Apply staged advanced prune after user confirmation.
    ApplyAdvancedPrune,

    /// Show an informational toast/message in the transcript.
    ShowInfoMessage(String),

    /// Open the approval popup.
    FullScreenApprovalRequest(ApprovalRequest),

    // (upstream event placeholders removed; handled by our prune events above)
}
