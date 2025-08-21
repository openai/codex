use codex_core::protocol::Event;
use codex_file_search::FileMatch;
use crossterm::event::KeyEvent;
use ratatui::text::Line;

use crate::app::ChatWidgetArgs;
use crate::slash_command::SlashCommand;
use std::path::PathBuf;

#[allow(clippy::large_enum_variant)]
pub(crate) enum AppEvent {
    CodexEvent(Event),

    /// Request a redraw which will be debounced by the [`App`].
    RequestRedraw,

    /// Actually draw the next frame.
    Redraw,

    KeyEvent(KeyEvent),

    /// Text pasted from the terminal clipboard.
    Paste(String),

    /// Request to exit the application gracefully.
    ExitRequest,

    /// Forward an `Op` to the Agent. Using an `AppEvent` for this avoids
    /// bubbling channels through layers of widgets.
    CodexOp(codex_core::protocol::Op),

    /// Latest formatted log line emitted by `tracing`.
    LatestLog(String),

    /// Dispatch a recognized slash command from the UI (composer) to the app
    /// layer so it can be handled centrally. Optional args carry any text that
    /// followed the command on the first line.
    DispatchCommand {
        command: SlashCommand,
        args: Option<String>,
    },

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

    InsertHistory(Vec<Line<'static>>),

    /// User selected a rollout file to resume from.
    ResumeSelected(PathBuf),

    /// User selected a past input to branch from. Contains the path to a
    /// truncated rollout file to resume from and the text to prefill in the
    /// composer for editing.
    BranchToPastInput {
        resume_path: PathBuf,
        prefill_text: String,
    },

    /// Onboarding: result of login_with_chatgpt.
    OnboardingAuthComplete(Result<(), String>),
    OnboardingComplete(ChatWidgetArgs),
}
