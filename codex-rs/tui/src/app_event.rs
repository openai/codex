use codex_core::protocol::Event;
use codex_file_search::FileMatch;
use crossterm::event::KeyEvent;
use ratatui::text::Line;

use crate::slash_command::SlashCommand;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;

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
    /// layer so it can be handled centrally. Optional `args` contains the
    /// left-trimmed raw argument string following the command, if any.
    DispatchCommand {
        cmd: SlashCommand,
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

    /// User selected a model from the model-selection dropdown.
    SelectModel(String),

    /// Request the app to open the model selector (populate options and show popup).
    OpenModelSelector,

    /// User selected an execution mode (approval + sandbox) from the dropdown or via /approvals.
    SelectExecutionMode {
        approval: AskForApproval,
        sandbox: SandboxPolicy,
    },

    /// Request the app to open the execution-mode selector (populate options and show popup).
    OpenExecutionSelector,
}
