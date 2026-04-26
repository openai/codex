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

use codex_app_server_protocol::AddCreditsNudgeCreditType;
use codex_app_server_protocol::AddCreditsNudgeEmailStatus;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_file_search::FileMatch;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::protocol::GetHistoryEntryResponseEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_approval_presets::ApprovalPreset;

use crate::bottom_pane::ApprovalRequest;
use crate::bottom_pane::StatusLineItem;
use crate::bottom_pane::TerminalTitleItem;
use crate::chatwidget::UserMessage;
use codex_config::types::ApprovalsReviewer;
use codex_features::Feature;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_realtime_webrtc::RealtimeWebrtcEvent;
use codex_realtime_webrtc::RealtimeWebrtcSessionHandle;

use crate::history_cell::HistoryCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RealtimeAudioDeviceKind {
    Microphone,
    Speaker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThreadGoalSetMode {
    ConfirmIfExists,
    ReplaceExisting,
}

impl RealtimeAudioDeviceKind {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Microphone => "Microphone",
            Self::Speaker => "Speaker",
        }
    }

    pub(crate) fn noun(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Speaker => "speaker",
        }
    }
}

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

/// Distinguishes why a rate-limit refresh was requested so the completion
/// handler can route the result correctly.
///
/// A `StartupPrefetch` fires once, concurrently with the rest of TUI init, and
/// only updates the cached snapshots (no status card to finalize). A
/// `StatusCommand` is tied to a specific `/status` invocation and must call
/// `finish_status_rate_limit_refresh` when done so the card stops showing a
/// "refreshing" state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RateLimitRefreshOrigin {
    /// Eagerly fetched after bootstrap so the first `/status` already has data.
    StartupPrefetch,
    /// User-initiated via `/status`; the `request_id` correlates with the
    /// status card that should be updated when the fetch completes.
    StatusCommand { request_id: u64 },
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    /// Open the agent picker for switching active threads.
    OpenAgentPicker,
    /// Switch the active thread to the selected agent.
    SelectAgentThread(ThreadId),

    /// Fork the current thread into a transient side conversation.
    StartSide {
        parent_thread_id: ThreadId,
        user_message: Option<UserMessage>,
    },

    /// Submit an op to the specified thread, regardless of current focus.
    SubmitThreadOp {
        thread_id: ThreadId,
        op: Op,
    },

    /// Deliver a synthetic history lookup response to a specific thread channel.
    ThreadHistoryEntryResponse {
        thread_id: ThreadId,
        event: GetHistoryEntryResponseEvent,
    },

    /// Start a new session.
    NewSession,

    /// Clear the terminal UI (screen + scrollback), start a fresh session, and keep the
    /// previous chat resumable.
    ClearUi,

    /// Clear the current context, start a fresh session, and submit an initial user message.
    ///
    /// This is the Plan Mode handoff path: the previous thread remains resumable, but the model
    /// sees only the explicit prompt carried in `text` once the new session is configured.
    ClearUiAndSubmitUserMessage {
        text: String,
    },

    /// Open the resume picker inside the running TUI session.
    OpenResumePicker,

    /// Resume a thread by UUID or thread name inside the running TUI session.
    ResumeSessionByIdOrName(String),

    /// Fork the current session into a new thread.
    ForkCurrentSession {
        placement: Option<ForkPanePlacement>,
    },

    /// Request to exit the application.
    ///
    /// Use `ShutdownFirst` for user-initiated quits so core cleanup runs and the
    /// UI exits only after `ShutdownComplete`. `Immediate` is a last-resort
    /// escape hatch that skips shutdown and may drop in-flight work (e.g.,
    /// background tasks, rollout flush, or child process cleanup).
    Exit(ExitMode),

    /// Request app-server account logout, then exit after it succeeds.
    Logout,

    /// Request to exit the application due to a fatal error.
    #[allow(dead_code)]
    FatalExitRequest(String),

    /// Forward an `Op` to the Agent. Using an `AppEvent` for this avoids
    /// bubbling channels through layers of widgets.
    CodexOp(Op),

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

    /// Refresh account rate limits in the background.
    RefreshRateLimits {
        origin: RateLimitRefreshOrigin,
    },

    /// Open the current thread goal summary/action menu.
    OpenThreadGoalMenu {
        thread_id: ThreadId,
    },

    /// Set or replace the current thread goal objective.
    SetThreadGoalObjective {
        thread_id: ThreadId,
        objective: String,
        mode: ThreadGoalSetMode,
    },

    /// Pause or unpause the current thread goal.
    SetThreadGoalStatus {
        thread_id: ThreadId,
        status: ThreadGoalStatus,
    },

    /// Clear the current thread goal.
    ClearThreadGoal {
        thread_id: ThreadId,
    },

    /// Result of refreshing rate limits.
    RateLimitsLoaded {
        origin: RateLimitRefreshOrigin,
        result: Result<Vec<RateLimitSnapshot>, String>,
    },

    /// Send a user-confirmed request to notify the workspace owner.
    SendAddCreditsNudgeEmail {
        credit_type: AddCreditsNudgeCreditType,
    },

    /// Result of notifying the workspace owner.
    AddCreditsNudgeEmailFinished {
        result: Result<AddCreditsNudgeEmailStatus, String>,
    },

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

    /// Fetch plugin marketplace state for the provided working directory.
    FetchPluginsList {
        cwd: PathBuf,
    },

    /// Result of fetching plugin marketplace state.
    PluginsLoaded {
        cwd: PathBuf,
        result: Result<PluginListResponse, String>,
    },

    /// Replace the plugins popup with a plugin-detail loading state.
    OpenPluginDetailLoading {
        plugin_display_name: String,
    },

    /// Fetch detail for a specific plugin from a marketplace.
    FetchPluginDetail {
        cwd: PathBuf,
        params: PluginReadParams,
    },

    /// Result of fetching plugin detail.
    PluginDetailLoaded {
        cwd: PathBuf,
        result: Result<PluginReadResponse, String>,
    },

    /// Replace the plugins popup with an install loading state.
    OpenPluginInstallLoading {
        plugin_display_name: String,
    },

    /// Replace the plugins popup with an uninstall loading state.
    OpenPluginUninstallLoading {
        plugin_display_name: String,
    },

    /// Install a specific plugin from a marketplace.
    FetchPluginInstall {
        cwd: PathBuf,
        marketplace_path: AbsolutePathBuf,
        plugin_name: String,
        plugin_display_name: String,
    },

    /// Result of installing a plugin.
    PluginInstallLoaded {
        cwd: PathBuf,
        marketplace_path: AbsolutePathBuf,
        plugin_name: String,
        plugin_display_name: String,
        result: Result<PluginInstallResponse, String>,
    },

    /// Uninstall a specific plugin by canonical plugin id.
    FetchPluginUninstall {
        cwd: PathBuf,
        plugin_id: String,
        plugin_display_name: String,
    },

    /// Result of uninstalling a plugin.
    PluginUninstallLoaded {
        cwd: PathBuf,
        plugin_id: String,
        plugin_display_name: String,
        result: Result<PluginUninstallResponse, String>,
    },

    /// Enable or disable an installed plugin.
    SetPluginEnabled {
        cwd: PathBuf,
        plugin_id: String,
        enabled: bool,
    },

    /// Result of enabling or disabling a plugin.
    PluginEnabledSet {
        cwd: PathBuf,
        plugin_id: String,
        enabled: bool,
        result: Result<(), String>,
    },

    /// Refresh plugin mention bindings from the current config.
    RefreshPluginMentions,

    /// Result of refreshing plugin mention bindings.
    PluginMentionsLoaded {
        plugins: Option<Vec<PluginCapabilitySummary>>,
    },

    /// Advance the post-install plugin app-auth flow.
    PluginInstallAuthAdvance {
        refresh_connectors: bool,
    },

    /// Abandon the post-install plugin app-auth flow.
    PluginInstallAuthAbandon,

    /// Fetch MCP inventory via app-server RPCs and render it into history.
    FetchMcpInventory {
        detail: McpServerStatusDetail,
    },

    /// Result of fetching MCP inventory via app-server RPCs.
    McpInventoryLoaded {
        result: Result<Vec<McpServerStatus>, String>,
        detail: McpServerStatusDetail,
    },

    /// Result of the startup skills refresh that runs after the first frame is scheduled.
    ///
    /// This event is startup-only. Interactive skills refreshes are handled synchronously through the app
    /// command path because those callers expect the visible skill state to be current when their command
    /// completes.
    SkillsListLoaded {
        result: Result<SkillsListResponse, String>,
    },

    /// Begin buffering initial resume replay rows before they are written to scrollback.
    BeginInitialHistoryReplayBuffer,

    InsertHistoryCell(Box<dyn HistoryCell>),

    /// Finish buffering initial resume replay after all replay events have been queued.
    EndInitialHistoryReplayBuffer,

    /// Replace the contiguous run of streaming `AgentMessageCell`s at the end of
    /// the transcript with a single `AgentMarkdownCell` that stores the raw
    /// markdown source and re-renders from it on resize.
    ///
    /// Emitted by `ChatWidget::flush_answer_stream_with_separator` after stream
    /// finalization. The `App` handler walks backward through `transcript_cells`
    /// to find the `AgentMessageCell` run and splices in the consolidated cell.
    /// The `cwd` keeps local file-link display stable across the final re-render.
    ConsolidateAgentMessage {
        source: String,
        cwd: PathBuf,
    },

    /// Replace the contiguous run of streaming `ProposedPlanStreamCell`s at the
    /// end of the transcript with a single source-backed `ProposedPlanCell`.
    ///
    /// Emitted by `ChatWidget::on_plan_item_completed` after plan stream
    /// finalization.
    ConsolidateProposedPlan(String),

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

    /// Persist the selected service tier to the appropriate config.
    PersistServiceTierSelection {
        service_tier: Option<ServiceTier>,
    },

    /// Open the device picker for a realtime microphone or speaker.
    OpenRealtimeAudioDeviceSelection {
        kind: RealtimeAudioDeviceKind,
    },

    /// Persist the selected realtime microphone or speaker to top-level config.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    PersistRealtimeAudioDeviceSelection {
        kind: RealtimeAudioDeviceKind,
        name: Option<String>,
    },

    /// Restart the selected realtime microphone or speaker locally.
    RestartRealtimeAudioDevice {
        kind: RealtimeAudioDeviceKind,
    },

    /// Result of creating a TUI-owned realtime WebRTC offer.
    RealtimeWebrtcOfferCreated {
        result: Result<RealtimeWebrtcOffer, String>,
    },

    /// Peer-connection lifecycle event from a TUI-owned realtime WebRTC session.
    RealtimeWebrtcEvent(RealtimeWebrtcEvent),

    /// Local microphone level from a TUI-owned realtime WebRTC session.
    RealtimeWebrtcLocalAudioLevel(u16),

    /// Open the reasoning selection popup after picking a model.
    OpenReasoningPopup {
        model: ModelPreset,
    },

    /// Open the Plan-mode reasoning scope prompt for the selected model/effort.
    OpenPlanReasoningScopePrompt {
        model: String,
        effort: Option<ReasoningEffort>,
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

    /// Update the current approvals reviewer in the running app and widget.
    UpdateApprovalsReviewer(ApprovalsReviewer),

    /// Update feature flags and persist them to the top-level config.
    UpdateFeatureFlags {
        updates: Vec<(Feature, bool)>,
    },

    /// Update memory settings and persist them to config.toml.
    UpdateMemorySettings {
        use_memories: bool,
        generate_memories: bool,
    },

    /// Clear all persisted local memory artifacts via the app-server.
    ResetMemories,

    /// Update whether the full access warning prompt has been acknowledged.
    UpdateFullAccessWarningAcknowledged(bool),

    /// Update whether the world-writable directories warning has been acknowledged.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    UpdateWorldWritableWarningAcknowledged(bool),

    /// Update whether the rate limit switch prompt has been acknowledged for the session.
    UpdateRateLimitSwitchPromptHidden(bool),

    /// Update the Plan-mode-specific reasoning effort in memory.
    UpdatePlanModeReasoningEffort(Option<ReasoningEffort>),

    /// Persist the acknowledgement flag for the full access warning prompt.
    PersistFullAccessWarningAcknowledged,

    /// Persist the acknowledgement flag for the world-writable directories warning.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    PersistWorldWritableWarningAcknowledged,

    /// Persist the acknowledgement flag for the rate limit switch prompt.
    PersistRateLimitSwitchPromptHidden,

    /// Persist the Plan-mode-specific reasoning effort.
    PersistPlanModeReasoningEffort(Option<ReasoningEffort>),

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
        path: AbsolutePathBuf,
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

    /// Live update for the in-progress voice recording placeholder. Carries
    /// the placeholder `id` and the text to display (e.g., an ASCII meter).
    #[cfg(not(target_os = "linux"))]
    UpdateRecordingMeter {
        id: String,
        text: String,
    },

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

    /// Submit feedback for the current thread via the app-server feedback RPC.
    SubmitFeedback {
        category: FeedbackCategory,
        reason: Option<String>,
        turn_id: Option<String>,
        include_logs: bool,
    },

    /// Result of a feedback upload request initiated by the TUI.
    FeedbackSubmitted {
        origin_thread_id: Option<ThreadId>,
        category: FeedbackCategory,
        include_logs: bool,
        result: Result<String, String>,
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

    /// Apply a user-confirmed terminal-title item ordering/selection.
    TerminalTitleSetup {
        items: Vec<TerminalTitleItem>,
    },
    /// Apply a temporary terminal-title preview while the setup UI is open.
    TerminalTitleSetupPreview {
        items: Vec<TerminalTitleItem>,
    },
    /// Dismiss the terminal-title setup UI without changing config.
    TerminalTitleSetupCancelled,

    /// Apply a user-confirmed syntax theme selection.
    SyntaxThemeSelected {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForkPanePlacement {
    Left,
    Right,
    Up,
    Down,
    Float,
}

#[derive(Debug)]
pub(crate) struct RealtimeWebrtcOffer {
    pub(crate) offer_sdp: String,
    pub(crate) handle: RealtimeWebrtcSessionHandle,
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
