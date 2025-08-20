use codex_core::protocol::Event;
use codex_file_search::FileMatch;
use crossterm::event::KeyEvent;
use ratatui::text::Line;
use std::time::Duration;

use crate::app::ChatWidgetArgs;
use crate::slash_command::SlashCommand;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    CodexEvent(Event),

    /// Request a redraw which will be debounced by the [`App`].
    RequestRedraw,

    /// Actually draw the next frame.
    Redraw,

    /// Schedule a one-shot animation frame roughly after the given duration.
    /// Multiple requests are coalesced by the central frame scheduler.
    ScheduleFrameIn(Duration),

    KeyEvent(KeyEvent),

    /// Text pasted from the terminal clipboard.
    Paste(String),

    /// Request to exit the application gracefully.
    ExitRequest,

    /// Forward an `Op` to the Agent. Using an `AppEvent` for this avoids
    /// bubbling channels through layers of widgets.
    CodexOp(codex_core::protocol::Op),

    /// Dispatch a recognized slash command from the UI (composer) to the app
    /// layer so it can be handled centrally.
    DispatchCommand(SlashCommand),

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

    InsertHistory(Vec<Line<'static>>),

    StartCommitAnimation,
    StopCommitAnimation,
    CommitTick,

    /// Onboarding: result of login_with_chatgpt.
    OnboardingAuthComplete(Result<(), String>),
    OnboardingComplete(ChatWidgetArgs),

    /// Update the current reasoning effort in the running app and widget.
    UpdateReasoningEffort(ReasoningEffort),

    /// Update the current model slug in the running app and widget.
    UpdateModel(String),

    /// Update the current approval policy in the running app and widget.
    UpdateAskForApprovalPolicy(AskForApproval),

    /// Update the current sandbox policy in the running app and widget.
    UpdateSandboxPolicy(SandboxPolicy),
}
