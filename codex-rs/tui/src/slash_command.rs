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
    #[strum(to_string = "stop", serialize = "clean")]
    Stop,
    Clear,
    Personality,
    Realtime,
    Settings,
    TestApproval,
    #[strum(serialize = "subagents", serialize = "multi-agents")]
    MultiAgents,
    // Debugging commands.
    #[strum(serialize = "debug-m-drop")]
    MemoryDrop,
    #[strum(serialize = "debug-m-update")]
    MemoryUpdate,
}

impl SlashCommand {
    fn spec(self) -> SlashCommandSpec {
        match self {
            SlashCommand::Help => SlashCommandSpec {
                description: "show slash command help",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Model => SlashCommandSpec {
                description: "choose what model and reasoning effort to use",
                help_forms: &[
                    "",
                    "<model> [default|none|minimal|low|medium|high|xhigh] [plan-only|all-modes]",
                ],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Fast => SlashCommandSpec {
                description: "toggle Fast mode to enable fastest inference at 2X plan usage",
                help_forms: &["", "<on|off|status>"],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Approvals => SlashCommandSpec {
                description: "choose what Codex is allowed to do",
                help_forms: &[
                    "",
                    "<read-only|auto|full-access> [--smart-approvals] [--confirm-full-access] [--remember-full-access] [--confirm-world-writable] [--remember-world-writable] [--enable-windows-sandbox=elevated|legacy]",
                ],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: false,
            },
            SlashCommand::Permissions => SlashCommandSpec {
                description: "choose what Codex is allowed to do",
                help_forms: &[
                    "",
                    "<read-only|auto|full-access> [--smart-approvals] [--confirm-full-access] [--remember-full-access] [--confirm-world-writable] [--remember-world-writable] [--enable-windows-sandbox=elevated|legacy]",
                ],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::ElevateSandbox => SlashCommandSpec {
                description: "set up elevated agent sandbox",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::SandboxReadRoot => SlashCommandSpec {
                description: "let sandbox read a directory: /sandbox-add-read-dir <absolute_path>",
                help_forms: &["<absolute-directory-path>"],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Experimental => SlashCommandSpec {
                description: "toggle experimental features",
                help_forms: &["", "<feature-key>=on|off ..."],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Skills => SlashCommandSpec {
                description: "use skills to improve how Codex performs specific tasks",
                help_forms: &["", "<list|manage>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Review => SlashCommandSpec {
                description: "review my current changes and find issues",
                help_forms: &[
                    "",
                    "uncommitted",
                    "branch <name>",
                    "commit <sha> [title]",
                    "<instructions>",
                ],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Rename => SlashCommandSpec {
                description: "rename the current thread",
                help_forms: &["", "<title...>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::New => SlashCommandSpec {
                description: "start a new chat during a conversation",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Resume => SlashCommandSpec {
                description: "resume a saved chat",
                help_forms: &["", "<thread-id>", "<thread-id> --path <rollout-path>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Fork => SlashCommandSpec {
                description: "fork the current chat",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Init => SlashCommandSpec {
                description: "create an AGENTS.md file with instructions for Codex",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::JustLikeUserMessage,
                show_in_command_popup: true,
            },
            SlashCommand::Compact => SlashCommandSpec {
                description: "summarize conversation to prevent hitting the context limit",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Plan => SlashCommandSpec {
                description: "switch to Plan mode",
                help_forms: &["", "<prompt...>"],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::JustLikeUserMessage,
                show_in_command_popup: true,
            },
            SlashCommand::Collab => SlashCommandSpec {
                description: "change collaboration mode (experimental)",
                help_forms: &["", "<default|plan>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Agent => SlashCommandSpec {
                description: "switch the active agent thread",
                help_forms: &["", "<thread-id>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Diff => SlashCommandSpec {
                description: "show git diff (including untracked files)",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Copy => SlashCommandSpec {
                description: "copy the latest Codex output to your clipboard",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Mention => SlashCommandSpec {
                description: "mention a file",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Status => SlashCommandSpec {
                description: "show current session configuration and token usage",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::DebugConfig => SlashCommandSpec {
                description: "show config layers and requirement sources for debugging",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Statusline => SlashCommandSpec {
                description: "configure which items appear in the status line",
                help_forms: &["", "<item-id>...", "none"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Theme => SlashCommandSpec {
                description: "choose a syntax highlighting theme",
                help_forms: &["", "<theme-name>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Mcp => SlashCommandSpec {
                description: "list configured MCP tools",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Apps => SlashCommandSpec {
                description: "manage apps",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Logout => SlashCommandSpec {
                description: "log out of Codex",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Quit => SlashCommandSpec {
                description: "exit Codex",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: false,
            },
            SlashCommand::Exit => SlashCommandSpec {
                description: "exit Codex",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Feedback => SlashCommandSpec {
                description: "send logs to maintainers",
                help_forms: &["", "<bug|bad-result|good-result|safety-check|other>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Rollout => SlashCommandSpec {
                description: "print the rollout file path",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Ps => SlashCommandSpec {
                description: "list background terminals",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Stop => SlashCommandSpec {
                description: "stop all background terminals",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Clear => SlashCommandSpec {
                description: "clear the terminal and start a new chat",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Personality => SlashCommandSpec {
                description: "choose a communication style for Codex",
                help_forms: &["", "<none|friendly|pragmatic>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::Realtime => SlashCommandSpec {
                description: "toggle realtime voice mode (experimental)",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::Settings => SlashCommandSpec {
                description: "configure realtime microphone/speaker",
                help_forms: &["", "<microphone|speaker> [default|<device-name>]"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::TestApproval => SlashCommandSpec {
                description: "test approval request",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::MultiAgents => SlashCommandSpec {
                description: "switch the active agent thread",
                help_forms: &["", "<thread-id>"],
                requires_interaction: true,
                execution_kind: SlashCommandExecutionKind::Immediate,
                show_in_command_popup: true,
            },
            SlashCommand::MemoryDrop => SlashCommandSpec {
                description: "DO NOT USE",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
            SlashCommand::MemoryUpdate => SlashCommandSpec {
                description: "DO NOT USE",
                help_forms: &[""],
                requires_interaction: false,
                execution_kind: SlashCommandExecutionKind::ChangesTurnContext,
                show_in_command_popup: true,
            },
        }
    }

    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        self.spec().description
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        match self {
            SlashCommand::MultiAgents => "subagents",
            _ => self.into(),
        }
    }

    /// Additional accepted built-in names besides `command()`.
    pub fn command_aliases(self) -> &'static [&'static str] {
        match self {
            SlashCommand::Help
            | SlashCommand::Model
            | SlashCommand::Fast
            | SlashCommand::Approvals
            | SlashCommand::Permissions
            | SlashCommand::ElevateSandbox
            | SlashCommand::SandboxReadRoot
            | SlashCommand::Experimental
            | SlashCommand::Skills
            | SlashCommand::Review
            | SlashCommand::Rename
            | SlashCommand::New
            | SlashCommand::Resume
            | SlashCommand::Fork
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Plan
            | SlashCommand::Collab
            | SlashCommand::Agent
            | SlashCommand::Diff
            | SlashCommand::Copy
            | SlashCommand::Mention
            | SlashCommand::Status
            | SlashCommand::DebugConfig
            | SlashCommand::Statusline
            | SlashCommand::Theme
            | SlashCommand::Mcp
            | SlashCommand::Apps
            | SlashCommand::Logout
            | SlashCommand::Quit
            | SlashCommand::Exit
            | SlashCommand::Feedback
            | SlashCommand::Rollout
            | SlashCommand::Ps
            | SlashCommand::Clear
            | SlashCommand::Personality
            | SlashCommand::Realtime
            | SlashCommand::Settings
            | SlashCommand::TestApproval
            | SlashCommand::MemoryDrop
            | SlashCommand::MemoryUpdate => &[],
            SlashCommand::Stop => &["clean"],
            SlashCommand::MultiAgents => &["multi-agents"],
        }
    }

    pub fn all_command_names(self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.command()).chain(self.command_aliases().iter().copied())
    }

    /// Human-facing forms accepted by the TUI.
    ///
    /// An empty string represents the bare `/command` form.
    pub fn help_forms(self) -> &'static [&'static str] {
        self.spec().help_forms
    }

    /// Whether bare dispatch opens interactive UI that should be resolved before queueing.
    pub fn requires_interaction(self) -> bool {
        self.spec().requires_interaction
    }

    /// How this command should behave when dispatched while another turn is running.
    pub fn execution_kind(self) -> SlashCommandExecutionKind {
        self.spec().execution_kind
    }

    pub fn show_in_command_popup(self) -> bool {
        self.spec().show_in_command_popup
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

#[derive(Clone, Copy)]
struct SlashCommandSpec {
    description: &'static str,
    help_forms: &'static [&'static str],
    requires_interaction: bool,
    execution_kind: SlashCommandExecutionKind,
    show_in_command_popup: bool,
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter()
        .filter(|command| command.is_visible())
        .flat_map(|command| command.all_command_names().map(move |name| (name, command)))
        .collect()
}

/// Return all visible built-in commands once each, in presentation order.
pub fn visible_built_in_slash_commands() -> Vec<SlashCommand> {
    SlashCommand::iter()
        .filter(|command| command.is_visible())
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    use super::SlashCommand;

    #[test]
    fn stop_command_is_canonical_name() {
        assert_eq!(SlashCommand::Stop.command(), "stop");
    }

    #[test]
    fn clean_alias_parses_to_stop_command() {
        assert_eq!(SlashCommand::from_str("clean"), Ok(SlashCommand::Stop));
    }
}
