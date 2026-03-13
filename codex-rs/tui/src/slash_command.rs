use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

/// Commands that can be invoked by starting a message with a leading slash.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, AsRefStr, IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // DO NOT ALPHA-SORT! Enum order is presentation order in the popup, so
    // more frequently used commands should be listed first.
    Help,
    Model,
    Fast,
    Approvals,
    Permissions,
    #[strum(serialize = "setup-default-sandbox")]
    ElevateSandbox,
    #[strum(serialize = "sandbox-add-read-dir")]
    SandboxReadRoot,
    Experimental,
    Skills,
    Review,
    Rename,
    New,
    Resume,
    Fork,
    Init,
    Compact,
    Plan,
    Collab,
    Agent,
    // Undo,
    Diff,
    Copy,
    Mention,
    Status,
    DebugConfig,
    Statusline,
    Theme,
    Mcp,
    Apps,
    Logout,
    Quit,
    Exit,
    Feedback,
    Rollout,
    Ps,
    Clean,
    Clear,
    Personality,
    Realtime,
    Settings,
    TestApproval,
    #[strum(serialize = "subagents")]
    MultiAgents,
    // Debugging commands.
    #[strum(serialize = "debug-m-drop")]
    MemoryDrop,
    #[strum(serialize = "debug-m-update")]
    MemoryUpdate,
}

impl SlashCommand {
    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        match self {
            SlashCommand::Help => "show slash command help",
            SlashCommand::Feedback => "send logs to maintainers",
            SlashCommand::New => "start a new chat during a conversation",
            SlashCommand::Init => "create an AGENTS.md file with instructions for Codex",
            SlashCommand::Compact => "summarize conversation to prevent hitting the context limit",
            SlashCommand::Review => "review my current changes and find issues",
            SlashCommand::Rename => "rename the current thread",
            SlashCommand::Resume => "resume a saved chat",
            SlashCommand::Clear => "clear the terminal and start a new chat",
            SlashCommand::Fork => "fork the current chat",
            // SlashCommand::Undo => "ask Codex to undo a turn",
            SlashCommand::Quit | SlashCommand::Exit => "exit Codex",
            SlashCommand::Diff => "show git diff (including untracked files)",
            SlashCommand::Copy => "copy the latest Codex output to your clipboard",
            SlashCommand::Mention => "mention a file",
            SlashCommand::Skills => "use skills to improve how Codex performs specific tasks",
            SlashCommand::Status => "show current session configuration and token usage",
            SlashCommand::DebugConfig => "show config layers and requirement sources for debugging",
            SlashCommand::Statusline => "configure which items appear in the status line",
            SlashCommand::Theme => "choose a syntax highlighting theme",
            SlashCommand::Ps => "list background terminals",
            SlashCommand::Clean => "stop all background terminals",
            SlashCommand::MemoryDrop => "DO NOT USE",
            SlashCommand::MemoryUpdate => "DO NOT USE",
            SlashCommand::Model => "choose what model and reasoning effort to use",
            SlashCommand::Fast => "toggle Fast mode to enable fastest inference at 2X plan usage",
            SlashCommand::Personality => "choose a communication style for Codex",
            SlashCommand::Realtime => "toggle realtime voice mode (experimental)",
            SlashCommand::Settings => "configure realtime microphone/speaker",
            SlashCommand::Plan => "switch to Plan mode",
            SlashCommand::Collab => "change collaboration mode (experimental)",
            SlashCommand::Agent | SlashCommand::MultiAgents => "switch the active agent thread",
            SlashCommand::Approvals => "choose what Codex is allowed to do",
            SlashCommand::Permissions => "choose what Codex is allowed to do",
            SlashCommand::ElevateSandbox => "set up elevated agent sandbox",
            SlashCommand::SandboxReadRoot => {
                "let sandbox read a directory: /sandbox-add-read-dir <absolute_path>"
            }
            SlashCommand::Experimental => "toggle experimental features",
            SlashCommand::Mcp => "list configured MCP tools",
            SlashCommand::Apps => "manage apps",
            SlashCommand::Logout => "log out of Codex",
            SlashCommand::Rollout => "print the rollout file path",
            SlashCommand::TestApproval => "test approval request",
        }
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        self.into()
    }

    /// Human-facing forms accepted by the TUI.
    ///
    /// An empty string represents the bare `/command` form.
    pub fn help_forms(self) -> &'static [&'static str] {
        match self {
            SlashCommand::Help => &[""],
            SlashCommand::Model => &[
                "",
                "<model> [default|none|minimal|low|medium|high|xhigh] [plan-only|all-modes]",
            ],
            SlashCommand::Fast => &["", "<on|off|status>"],
            SlashCommand::Approvals | SlashCommand::Permissions => &[
                "",
                "<read-only|auto|full-access> [--confirm-full-access] [--remember-full-access] [--confirm-world-writable] [--remember-world-writable] [--enable-windows-sandbox=elevated|legacy]",
            ],
            SlashCommand::ElevateSandbox => &[""],
            SlashCommand::SandboxReadRoot => &["<absolute-directory-path>"],
            SlashCommand::Experimental => &["", "<feature-key>=on|off ..."],
            SlashCommand::Skills => &["", "<list|manage>"],
            SlashCommand::Review => &[
                "",
                "uncommitted",
                "branch <name>",
                "commit <sha> [title]",
                "<instructions>",
            ],
            SlashCommand::Rename => &["", "<title...>"],
            SlashCommand::New => &[""],
            SlashCommand::Resume => &["", "<thread-id>", "<thread-id> --path <rollout-path>"],
            SlashCommand::Fork => &[""],
            SlashCommand::Init => &[""],
            SlashCommand::Compact => &[""],
            SlashCommand::Plan => &["", "<prompt...>"],
            SlashCommand::Collab => &["", "<default|plan>"],
            SlashCommand::Agent | SlashCommand::MultiAgents => &["", "<thread-id>"],
            SlashCommand::Diff => &[""],
            SlashCommand::Copy => &[""],
            SlashCommand::Mention => &[""],
            SlashCommand::Status => &[""],
            SlashCommand::DebugConfig => &[""],
            SlashCommand::Statusline => &["", "<item-id>...", "none"],
            SlashCommand::Theme => &["", "<theme-name>"],
            SlashCommand::Mcp => &[""],
            SlashCommand::Apps => &[""],
            SlashCommand::Logout => &[""],
            SlashCommand::Quit | SlashCommand::Exit => &[""],
            SlashCommand::Feedback => &["", "<bug|bad-result|good-result|safety-check|other>"],
            SlashCommand::Rollout => &[""],
            SlashCommand::Ps => &[""],
            SlashCommand::Clean => &[""],
            SlashCommand::Clear => &[""],
            SlashCommand::Personality => &["", "<none|friendly|pragmatic>"],
            SlashCommand::Realtime => &[""],
            SlashCommand::Settings => &["", "<microphone|speaker> [default|<device-name>]"],
            SlashCommand::TestApproval => &[""],
            SlashCommand::MemoryDrop | SlashCommand::MemoryUpdate => &[""],
        }
    }

    /// Whether bare dispatch opens interactive UI that should be resolved before queueing.
    pub fn requires_interaction(self) -> bool {
        match self {
            SlashCommand::Help => false,
            SlashCommand::Feedback
            | SlashCommand::Resume
            | SlashCommand::Review
            | SlashCommand::Rename
            | SlashCommand::Model
            | SlashCommand::Settings
            | SlashCommand::Personality
            | SlashCommand::Collab
            | SlashCommand::Agent
            | SlashCommand::MultiAgents
            | SlashCommand::Approvals
            | SlashCommand::Permissions
            | SlashCommand::Experimental
            | SlashCommand::Skills
            | SlashCommand::Statusline
            | SlashCommand::Theme => true,
            SlashCommand::Fast
            | SlashCommand::ElevateSandbox
            | SlashCommand::SandboxReadRoot
            | SlashCommand::New
            | SlashCommand::Fork
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Plan
            | SlashCommand::Diff
            | SlashCommand::Copy
            | SlashCommand::Mention
            | SlashCommand::Status
            | SlashCommand::DebugConfig
            | SlashCommand::Mcp
            | SlashCommand::Apps
            | SlashCommand::Logout
            | SlashCommand::Quit
            | SlashCommand::Exit
            | SlashCommand::Rollout
            | SlashCommand::Ps
            | SlashCommand::Clean
            | SlashCommand::Clear
            | SlashCommand::Realtime
            | SlashCommand::TestApproval
            | SlashCommand::MemoryDrop
            | SlashCommand::MemoryUpdate => false,
        }
    }

    /// How this command should behave when dispatched while another turn is running.
    pub fn execution_kind(self) -> SlashCommandExecutionKind {
        match self {
            SlashCommand::Plan | SlashCommand::Init => {
                SlashCommandExecutionKind::JustLikeUserMessage
            }
            SlashCommand::Model
            | SlashCommand::Fast
            | SlashCommand::Approvals
            | SlashCommand::Permissions
            | SlashCommand::ElevateSandbox
            | SlashCommand::SandboxReadRoot
            | SlashCommand::Experimental
            | SlashCommand::Review
            | SlashCommand::New
            | SlashCommand::Resume
            | SlashCommand::Fork
            | SlashCommand::Compact
            | SlashCommand::Clear
            | SlashCommand::Logout
            | SlashCommand::Personality
            | SlashCommand::Statusline
            | SlashCommand::Theme
            | SlashCommand::MemoryDrop
            | SlashCommand::MemoryUpdate => SlashCommandExecutionKind::ChangesTurnContext,
            SlashCommand::Help
            | SlashCommand::Skills
            | SlashCommand::Rename
            | SlashCommand::Collab
            | SlashCommand::Agent
            | SlashCommand::MultiAgents
            | SlashCommand::Diff
            | SlashCommand::Copy
            | SlashCommand::Mention
            | SlashCommand::Status
            | SlashCommand::DebugConfig
            | SlashCommand::Mcp
            | SlashCommand::Apps
            | SlashCommand::Quit
            | SlashCommand::Exit
            | SlashCommand::Feedback
            | SlashCommand::Rollout
            | SlashCommand::Ps
            | SlashCommand::Clean
            | SlashCommand::Realtime
            | SlashCommand::Settings
            | SlashCommand::TestApproval => SlashCommandExecutionKind::Immediate,
        }
    }

    fn is_visible(self) -> bool {
        match self {
            SlashCommand::SandboxReadRoot => cfg!(target_os = "windows"),
            SlashCommand::Copy => !cfg!(target_os = "android"),
            SlashCommand::Rollout | SlashCommand::TestApproval => cfg!(debug_assertions),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandExecutionKind {
    /// Behaves like a normal user message.
    ///
    /// Enter should submit immediately when idle, and queue while a turn is running.
    /// Use this for commands whose effect is "ask the model to do work now".
    JustLikeUserMessage,

    /// Does not become a user message, but changes state that affects future turns.
    ///
    /// While a turn is running, it must queue and apply later in order.
    ChangesTurnContext,

    /// Does not submit model work and does not need to wait for the current turn.
    ///
    /// Run it immediately, even while a turn is in progress.
    Immediate,
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter()
        .filter(|command| command.is_visible())
        .map(|c| (c.command(), c))
        .collect()
}
