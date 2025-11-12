use std::path::PathBuf;

use codex_common::approval_presets::ApprovalPreset;
use codex_common::model_presets::ModelPreset;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::Event;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_file_search::FileMatch;
use tokio::sync::oneshot;

use crate::bottom_pane::ApprovalRequest;
use crate::history_cell::HistoryCell;
use crate::security_review::SecurityReviewFailure;
use crate::security_review::SecurityReviewMetadata;
use crate::security_review::SecurityReviewMode;
use crate::security_review::SecurityReviewResult;

#[derive(Clone, Debug)]
pub(crate) struct SecurityReviewAutoScopeSelection {
    pub display_path: String,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SecurityReviewCommandState {
    Running,
    Matches,
    NoMatches,
    Error,
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
        model: ModelPreset,
    },

    /// Open the confirmation prompt before enabling full access mode.
    OpenFullAccessConfirmation {
        preset: ApprovalPreset,
    },

    /// Open the Windows world-writable directories warning.
    /// If `preset` is `Some`, the confirmation will apply the provided
    /// approval/sandbox configuration on Continue; if `None`, it performs no
    /// policy change and only acknowledges/dismisses the warning.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    OpenWorldWritableWarningConfirmation {
        preset: Option<ApprovalPreset>,
        /// Up to 3 sample world-writable directories to display in the warning.
        sample_paths: Vec<String>,
        /// If there are more than `sample_paths`, this carries the remaining count.
        extra_count: usize,
        /// True when the scan failed (e.g. ACL query error) and protections could not be verified.
        failed_scan: bool,
    },

    /// Show Windows Subsystem for Linux setup instructions for auto mode.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    ShowWindowsAutoModeInstructions,

    /// Update the current approval policy in the running app and widget.
    UpdateAskForApprovalPolicy(AskForApproval),

    /// Update the current sandbox policy in the running app and widget.
    UpdateSandboxPolicy(SandboxPolicy),

    /// Update whether the full access warning prompt has been acknowledged.
    UpdateFullAccessWarningAcknowledged(bool),

    /// Update whether the world-writable directories warning has been acknowledged.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    UpdateWorldWritableWarningAcknowledged(bool),

    /// Update whether the rate limit switch prompt has been acknowledged for the session.
    UpdateRateLimitSwitchPromptHidden(bool),

    /// Persist the acknowledgement flag for the full access warning prompt.
    PersistFullAccessWarningAcknowledged,

    /// Persist the acknowledgement flag for the world-writable directories warning.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    PersistWorldWritableWarningAcknowledged,

    /// Persist the acknowledgement flag for the rate limit switch prompt.
    PersistRateLimitSwitchPromptHidden,

    /// Skip the next world-writable scan (one-shot) after a user-confirmed continue.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    SkipNextWorldWritableScan,

    /// Re-open the approval presets popup.
    OpenApprovalsPopup,

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
    /// Open the feedback note entry overlay after the user selects a category.
    OpenFeedbackNote {
        category: FeedbackCategory,
        include_logs: bool,
    },

    /// Open the upload consent popup for feedback after selecting a category.
    OpenFeedbackConsent {
        category: FeedbackCategory,
    },

    /// Open the scoped path input for security reviews.
    OpenSecurityReviewPathPrompt(SecurityReviewMode),

    /// Begin running a security review with the given mode and optional scoped paths.
    StartSecurityReview {
        mode: SecurityReviewMode,
        include_paths: Vec<String>,
        scope_prompt: Option<String>,
        force_new: bool,
    },

    /// Resume a previously generated security review from disk.
    ResumeSecurityReview {
        output_root: PathBuf,
        metadata: SecurityReviewMetadata,
    },

    /// Prompt the user to confirm auto-detected scope selections.
    SecurityReviewAutoScopeConfirm {
        mode: SecurityReviewMode,
        prompt: String,
        selections: Vec<SecurityReviewAutoScopeSelection>,
        responder: oneshot::Sender<bool>,
    },

    /// Prompt the user to register at least two accounts and paste credentials.
    /// The responder receives `Some(raw_input)` when the user submits text, or `None` if dismissed.
    OpenRegistrationPrompt {
        url: Option<String>,
        responder: oneshot::Sender<Option<String>>,
    },

    /// Notify that the security review scope has been resolved to specific paths.
    SecurityReviewScopeResolved {
        paths: Vec<String>,
    },

    /// Update the command status display for running security review shell commands.
    SecurityReviewCommandStatus {
        id: u64,
        summary: String,
        state: SecurityReviewCommandState,
        preview: Vec<String>,
    },

    /// Append a progress log emitted during the security review.
    SecurityReviewLog(String),

    /// Security review completed successfully.
    SecurityReviewComplete {
        result: SecurityReviewResult,
    },

    /// Security review failed prior to completion.
    SecurityReviewFailed {
        error: SecurityReviewFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeedbackCategory {
    BadResult,
    GoodResult,
    Bug,
    Other,
}
