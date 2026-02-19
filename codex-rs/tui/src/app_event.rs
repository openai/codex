//! Application-level events used to coordinate UI actions.
//!
//! `AppEvent` is the internal message bus between UI components and the top-level `App` loop.
//! Widgets emit events to request actions that must be handled at the app layer (like opening
//! pickers, persisting configuration, or shutting down the agent), without needing direct access to
//! `App` internals.
//!
//! Exit is modelled explicitly via `AppEvent::Exit(ExitMode)` so callers can request shutdown-first
//! quits without reaching into the app loop or coupling to shutdown/exit sequencing.

use std::path::PathBuf;

use codex_chatgpt::connectors::AppInfo;
use codex_core::protocol::Event;
use codex_core::protocol::RateLimitSnapshot;
use codex_file_search::FileMatch;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ModelPreset;
use codex_utils_approval_presets::ApprovalPreset;

use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::StatusLineItem;
use crate::history_cell::HistoryCell;

use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::Personality;
use codex_protocol::openai_models::ReasoningEffort;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) enum WindowsSandboxEnableMode {
    Elevated,
    Legacy,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) struct ConnectorsSnapshot {
    pub(crate) connectors: Vec<AppInfo>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    CodexEvent(Event),
    /// Open the agent picker for switching active threads.
    OpenAgentPicker,
    /// Switch the active thread to the selected agent.
    SelectAgentThread(ThreadId),

    /// Start a new session.
    NewSession,

    /// Open the resume picker inside the running TUI session.
    OpenResumePicker,

    /// Fork the current session into a new thread.
    ForkCurrentSession,

    /// Request to exit the application.
    ///
    /// Use `ShutdownFirst` for user-initiated quits so core cleanup runs and the
    /// UI exits only after `ShutdownComplete`. `Immediate` is a last-resort
    /// escape hatch that skips shutdown and may drop in-flight work (e.g.,
    /// background tasks, rollout flush, or child process cleanup).
    Exit(ExitMode),

    /// Request to exit the application due to a fatal error.
    FatalExitRequest(String),

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

    /// Result of refreshing rate limits
    RateLimitSnapshotFetched(RateLimitSnapshot),

    /// Result of prefetching connectors.
    ConnectorsLoaded {
        result: Result<ConnectorsSnapshot, String>,
        is_final: bool,
    },

    /// Result of computing a `/diff` command.
    DiffResult(String),

    /// Open the app link view in the bottom pane.
    OpenAppLink {
        app_id: String,
        title: String,
        description: Option<String>,
        instructions: String,
        url: String,
        is_installed: bool,
        is_enabled: bool,
    },

    /// Open the provided URL in the user's browser.
    OpenUrlInBrowser {
        url: String,
    },

    /// Refresh app connector state and mention bindings.
    RefreshConnectors {
        force_refetch: bool,
    },

    InsertHistoryCell(Box<dyn HistoryCell>),

    /// Apply rollback semantics to local transcript cells.
    ///
    /// This is emitted when rollback was not initiated by the current
    /// backtrack flow so trimming occurs in AppEvent queue order relative to
    /// inserted history cells.
    ApplyThreadRollback {
        num_turns: u32,
    },

    StartCommitAnimation,
    StopCommitAnimation,
    CommitTick,

    /// Update the current reasoning effort in the running app and widget.
    UpdateReasoningEffort(Option<ReasoningEffort>),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Update the active collaboration mask in the running app and widget.
    UpdateCollaborationMode(CollaborationModeMask),

    /// Update the current personality in the running app and widget.
    UpdatePersonality(Personality),

    /// Persist the selected model and reasoning effort to the appropriate config.
    PersistModelSelection {
        model: String,
        effort: Option<ReasoningEffort>,
    },

    /// Persist the selected personality to the appropriate config.
    PersistPersonalitySelection {
        personality: Personality,
    },

    /// Open the reasoning selection popup after picking a model.
    OpenReasoningPopup {
        model: ModelPreset,
    },

    /// Open the full model picker (non-auto models).
    OpenAllModelsPopup {
        models: Vec<ModelPreset>,
    },

    /// Open the confirmation prompt before enabling full access mode.
    OpenFullAccessConfirmation {
        preset: ApprovalPreset,
        return_to_permissions: bool,
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

    /// Prompt to enable the Windows sandbox feature before using Agent mode.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    OpenWindowsSandboxEnablePrompt {
        preset: ApprovalPreset,
    },

    /// Open the Windows sandbox fallback prompt after declining or failing elevation.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    OpenWindowsSandboxFallbackPrompt {
        preset: ApprovalPreset,
    },

    /// Begin the elevated Windows sandbox setup flow.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    BeginWindowsSandboxElevatedSetup {
        preset: ApprovalPreset,
    },

    /// Begin the non-elevated Windows sandbox setup flow.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    BeginWindowsSandboxLegacySetup {
        preset: ApprovalPreset,
    },

    /// Begin a non-elevated grant of read access for an additional directory.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    BeginWindowsSandboxGrantReadRoot {
        path: String,
    },

    /// Result of attempting to grant read access for an additional directory.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    WindowsSandboxGrantReadRootCompleted {
        path: PathBuf,
        error: Option<String>,
    },

    /// Enable the Windows sandbox feature and switch to Agent mode.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    EnableWindowsSandboxForAgentMode {
        preset: ApprovalPreset,
        mode: WindowsSandboxEnableMode,
    },

    /// Update the Windows sandbox feature mode without changing approval presets.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]

    /// Update the current approval policy in the running app and widget.
    UpdateAskForApprovalPolicy(AskForApproval),

    /// Update the current sandbox policy in the running app and widget.
    UpdateSandboxPolicy(SandboxPolicy),

    /// Update feature flags and persist them to the top-level config.
    UpdateFeatureFlags {
        updates: Vec<(Feature, bool)>,
    },

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

    /// Persist the acknowledgement flag for the model migration prompt.
    PersistModelMigrationPromptAcknowledged {
        from_model: String,
        to_model: String,
    },

    /// Skip the next world-writable scan (one-shot) after a user-confirmed continue.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    SkipNextWorldWritableScan,

    /// Re-open the approval presets popup.
    OpenApprovalsPopup,

    /// Open the skills list popup.
    OpenSkillsList,

    /// Open the skills enable/disable picker.
    OpenManageSkillsPopup,

    /// Enable or disable a skill by path.
    SetSkillEnabled {
        path: PathBuf,
        enabled: bool,
    },

    /// Enable or disable an app by connector ID.
    SetAppEnabled {
        id: String,
        enabled: bool,
    },

    /// Notify that the manage skills popup was closed.
    ManageSkillsClosed,

    /// Re-open the permissions presets popup.
    OpenPermissionsPopup,

    /// Open the branch picker option from the review popup.
    OpenReviewBranchPicker(PathBuf),

    /// Open the commit picker option from the review popup.
    OpenReviewCommitPicker(PathBuf),

    /// Open the custom prompt option from the review popup.
    OpenReviewCustomPrompt,

    /// Submit a user message with an explicit collaboration mask.
    SubmitUserMessageWithMode {
        text: String,
        collaboration_mode: CollaborationModeMask,
    },

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

    /// Launch the external editor after a normal draw has completed.
    LaunchExternalEditor,

    /// Async update of the current git branch for status line rendering.
    StatusLineBranchUpdated {
        cwd: PathBuf,
        branch: Option<String>,
    },
    /// Apply a user-confirmed status-line item ordering/selection.
    StatusLineSetup {
        items: Vec<StatusLineItem>,
    },
    /// Dismiss the status-line setup UI without changing config.
    StatusLineSetupCancelled,
}

/// The exit strategy requested by the UI layer.
///
/// Most user-initiated exits should use `ShutdownFirst` so core cleanup runs and the UI exits only
/// after core acknowledges completion. `Immediate` is an escape hatch for cases where shutdown has
/// already completed (or is being bypassed) and the UI loop should terminate right away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitMode {
    /// Shutdown core and exit after completion.
    ShutdownFirst,
    /// Exit the UI loop immediately without waiting for shutdown.
    ///
    /// This skips `Op::Shutdown`, so any in-flight work may be dropped and
    /// cleanup that normally runs before `ShutdownComplete` can be missed.
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeedbackCategory {
    BadResult,
    GoodResult,
    Bug,
    SafetyCheck,
    Other,
}
