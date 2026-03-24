use strum_macros::AsRefStr;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

use crate::app_event::FeedbackCategory;
use crate::bottom_pane::StatusLineItem;
use crate::slash_command_protocol::SlashArgsSchema;
use crate::slash_command_protocol::SlashCommandParseInput;
use crate::slash_command_protocol::SlashCommandUsageErrorKind;
use crate::slash_command_protocol::SlashSerializedText;
pub(crate) use crate::slash_command_protocol::SlashTextArg;
use crate::slash_command_protocol::enum_choice;
use crate::slash_command_protocol::from_str_value;
use crate::slash_command_protocol::list;
use crate::slash_command_protocol::named_or_positional;
use crate::slash_command_protocol::positional;
use crate::slash_command_protocol::remainder;
use crate::slash_command_protocol::string;
use crate::slash_command_protocol::text;

/// Commands that can be invoked by starting a message with a leading slash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, AsRefStr, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // DO NOT ALPHA-SORT! Enum order is presentation order in the popup, so
    // more frequently used commands should be listed first.
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
    Diff,
    Copy,
    Mention,
    Status,
    DebugConfig,
    Title,
    Statusline,
    Theme,
    Mcp,
    Apps,
    Plugins,
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
    #[strum(serialize = "subagents")]
    MultiAgents,
    #[strum(serialize = "debug-m-drop")]
    MemoryDrop,
    #[strum(serialize = "debug-m-update")]
    MemoryUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlashCommandBareBehavior {
    DispatchesDirectly,
    OpensUi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SlashCommandUsageError {
    command: SlashCommand,
    kind: SlashCommandUsageErrorKind,
}

impl SlashCommandUsageError {
    pub(crate) fn message(self) -> String {
        let usage = self.command.usage_lines().join(" | ");
        match self.kind {
            SlashCommandUsageErrorKind::UnexpectedInlineArgs => format!(
                "'/{}' does not accept inline arguments. Usage: {usage}",
                self.command.command()
            ),
            SlashCommandUsageErrorKind::InvalidInlineArgs => format!("Usage: {usage}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FastSlashCommandArgs {
    On,
    Off,
    Status,
}

const FAST_MODE_CHOICES: &[(&str, FastSlashCommandArgs)] = &[
    ("on", FastSlashCommandArgs::On),
    ("off", FastSlashCommandArgs::Off),
    ("status", FastSlashCommandArgs::Status),
];

const FEEDBACK_CATEGORY_CHOICES: &[(&str, FeedbackCategory)] = &[
    ("bad-result", FeedbackCategory::BadResult),
    ("good-result", FeedbackCategory::GoodResult),
    ("bug", FeedbackCategory::Bug),
    ("safety-check", FeedbackCategory::SafetyCheck),
    ("other", FeedbackCategory::Other),
];

pub(crate) trait SlashCommandInlineArgs: Sized {
    const USAGE_LINES: &'static [&'static str];
    fn args_schema() -> Box<dyn SlashArgsSchema<Self>>;

    fn into_invocation(self) -> SlashCommandInvocation;

    fn parse_inline(input: SlashCommandParseInput<'_>) -> Result<Self, SlashCommandUsageErrorKind> {
        let args_schema = Self::args_schema();
        let mut parser = crate::slash_command_protocol::SlashArgsParser::new(input)?;
        let value = args_schema.parse(&mut parser)?;
        args_schema.finish(parser)?;
        Ok(value)
    }

    fn serialize_inline(&self) -> SlashSerializedText {
        let args_schema = Self::args_schema();
        let mut serializer = crate::slash_command_protocol::SlashArgsSerializer::default();
        args_schema.serialize(self, &mut serializer);
        serializer.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FastArgs {
    pub(crate) mode: FastSlashCommandArgs,
}

impl SlashCommandInlineArgs for FastArgs {
    const USAGE_LINES: &'static [&'static str] = &["/fast", "/fast [on|off|status]"];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(
            positional(enum_choice(FAST_MODE_CHOICES).ascii_case_insensitive())
                .map_result(|mode| Ok(Self { mode }), |args| args.mode),
        )
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::Fast(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RenameArgs {
    pub(crate) title: SlashTextArg,
}

impl SlashCommandInlineArgs for RenameArgs {
    const USAGE_LINES: &'static [&'static str] = &["/rename", "/rename <title>"];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(
            remainder(text()).map_result(|title| Ok(Self { title }), |args| args.title.clone()),
        )
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::Rename(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanArgs {
    pub(crate) prompt: SlashTextArg,
}

impl SlashCommandInlineArgs for PlanArgs {
    const USAGE_LINES: &'static [&'static str] = &["/plan", "/plan <prompt>"];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(
            remainder(text()).map_result(|prompt| Ok(Self { prompt }), |args| args.prompt.clone()),
        )
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::Plan(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReviewArgs {
    pub(crate) instructions: SlashTextArg,
}

impl SlashCommandInlineArgs for ReviewArgs {
    const USAGE_LINES: &'static [&'static str] = &["/review", "/review <instructions>"];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(remainder(text()).map_result(
            |instructions| Ok(Self { instructions }),
            |args| args.instructions.clone(),
        ))
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::Review(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SandboxReadRootArgs {
    pub(crate) path: String,
}

impl SlashCommandInlineArgs for SandboxReadRootArgs {
    const USAGE_LINES: &'static [&'static str] = &[
        "/sandbox-add-read-dir <absolute-path>",
        "/sandbox-add-read-dir --path=<absolute-path>",
    ];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(
            named_or_positional("path", string())
                .map_result(|path| Ok(Self { path }), |args| args.path.clone()),
        )
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::SandboxReadRoot(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeedbackArgs {
    pub(crate) category: FeedbackCategory,
}

impl SlashCommandInlineArgs for FeedbackArgs {
    const USAGE_LINES: &'static [&'static str] = &[
        "/feedback",
        "/feedback <bad-result|good-result|bug|safety-check|other>",
    ];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(
            positional(enum_choice(FEEDBACK_CATEGORY_CHOICES))
                .map_result(|category| Ok(Self { category }), |args| args.category),
        )
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::Feedback(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatuslineArgs {
    pub(crate) items: Vec<StatusLineItem>,
}

impl SlashCommandInlineArgs for StatuslineArgs {
    const USAGE_LINES: &'static [&'static str] = &["/statusline", "/statusline <item>..."];

    fn args_schema() -> Box<dyn SlashArgsSchema<Self>> {
        Box::new(list(from_str_value::<StatusLineItem>()).map_result(
            |items| {
                if items.is_empty() {
                    Err(SlashCommandUsageErrorKind::InvalidInlineArgs)
                } else {
                    Ok(Self { items })
                }
            },
            |args| args.items.clone(),
        ))
    }

    fn into_invocation(self) -> SlashCommandInvocation {
        SlashCommandInvocation::Statusline(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SlashCommandInvocation {
    Bare(SlashCommand),
    Fast(FastArgs),
    Rename(RenameArgs),
    Plan(PlanArgs),
    Review(ReviewArgs),
    SandboxReadRoot(SandboxReadRootArgs),
    Feedback(FeedbackArgs),
    Statusline(StatuslineArgs),
}

impl SlashCommandInvocation {
    pub(crate) fn bare(command: SlashCommand) -> Self {
        Self::Bare(command)
    }

    pub(crate) fn command(&self) -> SlashCommand {
        match self {
            Self::Bare(command) => *command,
            Self::Fast(_) => SlashCommand::Fast,
            Self::Rename(_) => SlashCommand::Rename,
            Self::Plan(_) => SlashCommand::Plan,
            Self::Review(_) => SlashCommand::Review,
            Self::SandboxReadRoot(_) => SlashCommand::SandboxReadRoot,
            Self::Feedback(_) => SlashCommand::Feedback,
            Self::Statusline(_) => SlashCommand::Statusline,
        }
    }

    pub(crate) fn serialize(&self) -> SlashSerializedText {
        let prefix = format!("/{}", self.command().command());
        match self {
            Self::Bare(_) => SlashSerializedText::empty().with_prefix(&prefix),
            Self::Fast(args) => args.serialize_inline().with_prefix(&prefix),
            Self::Rename(args) => args.serialize_inline().with_prefix(&prefix),
            Self::Plan(args) => args.serialize_inline().with_prefix(&prefix),
            Self::Review(args) => args.serialize_inline().with_prefix(&prefix),
            Self::SandboxReadRoot(args) => args.serialize_inline().with_prefix(&prefix),
            Self::Feedback(args) => args.serialize_inline().with_prefix(&prefix),
            Self::Statusline(args) => args.serialize_inline().with_prefix(&prefix),
        }
    }

    pub(crate) fn into_prefixed_string(self) -> String {
        self.serialize().text
    }
}

pub(crate) type SlashCommandInlineParser =
    for<'a> fn(
        SlashCommandParseInput<'a>,
    ) -> Result<SlashCommandInvocation, SlashCommandUsageErrorKind>;

pub(crate) struct SlashCommandSpec {
    pub(crate) command: SlashCommand,
    pub(crate) description: &'static str,
    pub(crate) available_during_task: bool,
    pub(crate) is_disabled: bool,
    pub(crate) hide_in_command_popup: bool,
    pub(crate) usage_lines: &'static [&'static str],
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) bare_behavior: SlashCommandBareBehavior,
    pub(crate) parse_inline: SlashCommandInlineParser,
}

fn reject_inline_args(
    _input: SlashCommandParseInput<'_>,
) -> Result<SlashCommandInvocation, SlashCommandUsageErrorKind> {
    Err(SlashCommandUsageErrorKind::UnexpectedInlineArgs)
}

fn parse_typed_inline<T>(
    input: SlashCommandParseInput<'_>,
) -> Result<SlashCommandInvocation, SlashCommandUsageErrorKind>
where
    T: SlashCommandInlineArgs,
{
    T::parse_inline(input).map(T::into_invocation)
}

// ===== /model =====
const MODEL_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Model,
    description: "choose what model and reasoning effort to use",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/model"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /fast =====
const FAST_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Fast,
    description: "toggle Fast mode to enable fastest inference at 2X plan usage",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: FastArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: parse_typed_inline::<FastArgs>,
};

// ===== /approvals =====
const APPROVALS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Approvals,
    description: "choose what Codex is allowed to do",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: true,
    usage_lines: &["/approvals"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /permissions =====
const PERMISSIONS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Permissions,
    description: "choose what Codex is allowed to do",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/permissions"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /setup-default-sandbox =====
const ELEVATE_SANDBOX_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::ElevateSandbox,
    description: "set up elevated agent sandbox",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/setup-default-sandbox"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /sandbox-add-read-dir =====
const SANDBOX_READ_ROOT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::SandboxReadRoot,
    description: "let sandbox read a directory: /sandbox-add-read-dir <absolute_path>",
    available_during_task: false,
    is_disabled: !cfg!(target_os = "windows"),
    hide_in_command_popup: false,
    usage_lines: SandboxReadRootArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: parse_typed_inline::<SandboxReadRootArgs>,
};

// ===== /experimental =====
const EXPERIMENTAL_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Experimental,
    description: "toggle experimental features",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/experimental"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /skills =====
const SKILLS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Skills,
    description: "use skills to improve how Codex performs specific tasks",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/skills"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /review =====
const REVIEW_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Review,
    description: "review my current changes and find issues",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: ReviewArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: parse_typed_inline::<ReviewArgs>,
};

// ===== /rename =====
const RENAME_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Rename,
    description: "rename the current thread",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: RenameArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: parse_typed_inline::<RenameArgs>,
};

// ===== /new =====
const NEW_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::New,
    description: "start a new chat during a conversation",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/new"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /resume =====
const RESUME_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Resume,
    description: "resume a saved chat",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/resume"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /fork =====
const FORK_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Fork,
    description: "fork the current chat",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/fork"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /init =====
const INIT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Init,
    description: "create an AGENTS.md file with instructions for Codex",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/init"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /compact =====
const COMPACT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Compact,
    description: "summarize conversation to prevent hitting the context limit",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/compact"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /plan =====
const PLAN_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Plan,
    description: "switch to Plan mode",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: PlanArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: parse_typed_inline::<PlanArgs>,
};

// ===== /collab =====
const COLLAB_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Collab,
    description: "change collaboration mode (experimental)",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/collab"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /agent =====
const AGENT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Agent,
    description: "switch the active agent thread",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/agent"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /diff =====
const DIFF_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Diff,
    description: "show git diff (including untracked files)",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/diff"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /copy =====
const COPY_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Copy,
    description: "copy the latest Codex output to your clipboard",
    available_during_task: true,
    is_disabled: cfg!(target_os = "android"),
    hide_in_command_popup: false,
    usage_lines: &["/copy"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /mention =====
const MENTION_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Mention,
    description: "mention a file",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/mention"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /status =====
const STATUS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Status,
    description: "show current session configuration and token usage",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/status"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /debug-config =====
const DEBUG_CONFIG_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::DebugConfig,
    description: "show config layers and requirement sources for debugging",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/debug-config"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /title =====
const TITLE_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Title,
    description: "configure which items appear in the terminal title",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/title"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /statusline =====
const STATUSLINE_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Statusline,
    description: "configure which items appear in the status line",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: StatuslineArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: parse_typed_inline::<StatuslineArgs>,
};

// ===== /theme =====
const THEME_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Theme,
    description: "choose a syntax highlighting theme",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/theme"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /mcp =====
const MCP_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Mcp,
    description: "list configured MCP tools",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/mcp"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /apps =====
const APPS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Apps,
    description: "manage apps",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/apps"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /plugins =====
const PLUGINS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Plugins,
    description: "browse plugins",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/plugins"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /logout =====
const LOGOUT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Logout,
    description: "log out of Codex",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/logout"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /quit =====
const QUIT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Quit,
    description: "exit Codex",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: true,
    usage_lines: &["/quit"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /exit =====
const EXIT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Exit,
    description: "exit Codex",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/exit"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /feedback =====
const FEEDBACK_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Feedback,
    description: "send logs to maintainers",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: FeedbackArgs::USAGE_LINES,
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: parse_typed_inline::<FeedbackArgs>,
};

// ===== /rollout =====
const ROLLOUT_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Rollout,
    description: "print the rollout file path",
    available_during_task: true,
    is_disabled: !cfg!(debug_assertions),
    hide_in_command_popup: false,
    usage_lines: &["/rollout"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /ps =====
const PS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Ps,
    description: "list background terminals",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/ps"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /stop =====
const STOP_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Stop,
    description: "stop all background terminals",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/stop"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /clear =====
const CLEAR_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Clear,
    description: "clear the terminal and start a new chat",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/clear"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /personality =====
const PERSONALITY_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Personality,
    description: "choose a communication style for Codex",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/personality"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /realtime =====
const REALTIME_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Realtime,
    description: "toggle realtime voice mode (experimental)",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/realtime"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /settings =====
const SETTINGS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::Settings,
    description: "configure realtime microphone/speaker",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/settings"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /test-approval =====
const TEST_APPROVAL_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::TestApproval,
    description: "test approval request",
    available_during_task: true,
    is_disabled: !cfg!(debug_assertions),
    hide_in_command_popup: false,
    usage_lines: &["/test-approval"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /subagents =====
const MULTI_AGENTS_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::MultiAgents,
    description: "switch the active agent thread",
    available_during_task: true,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/subagents"],
    bare_behavior: SlashCommandBareBehavior::OpensUi,
    parse_inline: reject_inline_args,
};

// ===== /debug-m-drop =====
const MEMORY_DROP_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::MemoryDrop,
    description: "DO NOT USE",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/debug-m-drop"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

// ===== /debug-m-update =====
const MEMORY_UPDATE_SPEC: SlashCommandSpec = SlashCommandSpec {
    command: SlashCommand::MemoryUpdate,
    description: "DO NOT USE",
    available_during_task: false,
    is_disabled: false,
    hide_in_command_popup: false,
    usage_lines: &["/debug-m-update"],
    bare_behavior: SlashCommandBareBehavior::DispatchesDirectly,
    parse_inline: reject_inline_args,
};

const SLASH_COMMAND_SPECS: &[SlashCommandSpec] = &[
    MODEL_SPEC,
    FAST_SPEC,
    APPROVALS_SPEC,
    PERMISSIONS_SPEC,
    ELEVATE_SANDBOX_SPEC,
    SANDBOX_READ_ROOT_SPEC,
    EXPERIMENTAL_SPEC,
    SKILLS_SPEC,
    REVIEW_SPEC,
    RENAME_SPEC,
    NEW_SPEC,
    RESUME_SPEC,
    FORK_SPEC,
    INIT_SPEC,
    COMPACT_SPEC,
    PLAN_SPEC,
    COLLAB_SPEC,
    AGENT_SPEC,
    DIFF_SPEC,
    COPY_SPEC,
    MENTION_SPEC,
    STATUS_SPEC,
    DEBUG_CONFIG_SPEC,
    TITLE_SPEC,
    STATUSLINE_SPEC,
    THEME_SPEC,
    MCP_SPEC,
    APPS_SPEC,
    PLUGINS_SPEC,
    LOGOUT_SPEC,
    QUIT_SPEC,
    EXIT_SPEC,
    FEEDBACK_SPEC,
    ROLLOUT_SPEC,
    PS_SPEC,
    STOP_SPEC,
    CLEAR_SPEC,
    PERSONALITY_SPEC,
    REALTIME_SPEC,
    SETTINGS_SPEC,
    TEST_APPROVAL_SPEC,
    MULTI_AGENTS_SPEC,
    MEMORY_DROP_SPEC,
    MEMORY_UPDATE_SPEC,
];

impl SlashCommand {
    fn spec(self) -> &'static SlashCommandSpec {
        match SLASH_COMMAND_SPECS.iter().find(|spec| spec.command == self) {
            Some(spec) => spec,
            None => panic!("every slash command must have a registered spec"),
        }
    }

    pub(crate) fn parse_invocation(
        self,
        args: &str,
        text_elements: &[codex_protocol::user_input::TextElement],
    ) -> Result<SlashCommandInvocation, SlashCommandUsageError> {
        if args.trim().is_empty() {
            return Ok(SlashCommandInvocation::Bare(self));
        }

        (self.spec().parse_inline)(SlashCommandParseInput {
            args,
            text_elements,
        })
        .map_err(|kind| SlashCommandUsageError {
            command: self,
            kind,
        })
    }

    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        self.spec().description
    }

    /// Command string without the leading '/'.
    pub fn command(self) -> &'static str {
        self.into()
    }

    /// User-visible usage forms for this command.
    pub(crate) fn usage_lines(self) -> &'static [&'static str] {
        self.spec().usage_lines
    }

    /// Whether this command can be run while a task is in progress.
    pub fn available_during_task(self) -> bool {
        self.spec().available_during_task
    }

    pub(crate) fn hide_in_command_popup(self) -> bool {
        self.spec().hide_in_command_popup
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SLASH_COMMAND_SPECS
        .iter()
        .filter(|spec| !spec.is_disabled)
        .map(|spec| (spec.command.command(), spec.command))
        .collect()
}

#[cfg(test)]
mod tests {
    use codex_protocol::user_input::ByteRange;
    use codex_protocol::user_input::TextElement;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use proptest::sample::select;
    use proptest::test_runner::Config as ProptestConfig;
    use proptest::test_runner::TestCaseError;
    use proptest::test_runner::TestRunner;
    use std::str::FromStr;

    use super::*;

    fn placeholder_text_arg(placeholder: &str) -> SlashTextArg {
        SlashTextArg::new(
            placeholder.to_string(),
            vec![TextElement::new(
                (0..placeholder.len()).into(),
                Some(placeholder.to_string()),
            )],
        )
    }

    fn shift_text_element_left(element: &TextElement, offset: usize) -> Option<TextElement> {
        if element.byte_range.end <= offset {
            return None;
        }
        let start = element.byte_range.start.saturating_sub(offset);
        let end = element.byte_range.end.saturating_sub(offset);
        (start < end).then(|| element.map_range(|_| ByteRange { start, end }))
    }

    fn serialized_args(
        invocation: &SlashCommandInvocation,
    ) -> (String, Vec<codex_protocol::user_input::TextElement>) {
        let serialized = invocation.serialize();
        let prefix = format!("/{}", invocation.command().command());
        let args = serialized
            .text
            .strip_prefix(&prefix)
            .expect("serialized invocation should start with command prefix");
        let Some(args) = args.strip_prefix(' ') else {
            return (String::new(), Vec::new());
        };
        let offset = prefix.len() + 1;
        let elements = serialized
            .text_elements
            .iter()
            .filter_map(|element| shift_text_element_left(element, offset))
            .collect();
        (args.to_string(), elements)
    }

    fn token_text_strategy() -> BoxedStrategy<String> {
        prop_oneof![
            proptest::string::string_regex("[A-Za-z0-9._/-]{1,16}").unwrap(),
            (
                proptest::string::string_regex("[A-Za-z0-9._/-]{1,8}").unwrap(),
                proptest::string::string_regex("[A-Za-z0-9._/-]{1,8}").unwrap(),
            )
                .prop_map(|(lhs, rhs)| format!("{lhs} {rhs}")),
            proptest::string::string_regex("[A-Za-z0-9._/-]{1,8}[\"'][A-Za-z0-9._/-]{1,8}")
                .unwrap(),
        ]
        .boxed()
    }

    fn plain_text_arg_strategy() -> BoxedStrategy<SlashTextArg> {
        proptest::string::string_regex(
            "[A-Za-z0-9][A-Za-z0-9._/'\"-]{0,10}( [A-Za-z0-9][A-Za-z0-9._/'\"-]{0,10}){0,3}",
        )
        .unwrap()
        .prop_map(|text| SlashTextArg::new(text, Vec::new()))
        .boxed()
    }

    fn placeholder_text_arg_strategy() -> BoxedStrategy<SlashTextArg> {
        (
            proptest::string::string_regex("[A-Za-z]{0,6}").unwrap(),
            select(vec!["[Image #1]".to_string(), "[Image #12]".to_string()]),
            proptest::string::string_regex("[A-Za-z]{0,6}").unwrap(),
        )
            .prop_map(|(prefix, placeholder, suffix)| {
                let mut text = String::new();
                if !prefix.is_empty() {
                    text.push_str(&prefix);
                    text.push(' ');
                }
                let start = text.len();
                text.push_str(&placeholder);
                let end = text.len();
                if !suffix.is_empty() {
                    text.push(' ');
                    text.push_str(&suffix);
                }
                SlashTextArg::new(
                    text,
                    vec![TextElement::new((start..end).into(), Some(placeholder))],
                )
            })
            .boxed()
    }

    fn text_arg_strategy() -> BoxedStrategy<SlashTextArg> {
        prop_oneof![plain_text_arg_strategy(), placeholder_text_arg_strategy(),].boxed()
    }

    fn string_arg_strategy() -> BoxedStrategy<String> {
        token_text_strategy().boxed()
    }

    impl SlashCommand {
        fn roundtrip_test_invocations(self) -> Vec<SlashCommandInvocation> {
            let bare = SlashCommandInvocation::Bare(self);
            match self {
                SlashCommand::Fast => vec![
                    bare,
                    SlashCommandInvocation::Fast(FastArgs {
                        mode: FastSlashCommandArgs::On,
                    }),
                    SlashCommandInvocation::Fast(FastArgs {
                        mode: FastSlashCommandArgs::Off,
                    }),
                    SlashCommandInvocation::Fast(FastArgs {
                        mode: FastSlashCommandArgs::Status,
                    }),
                ],
                SlashCommand::SandboxReadRoot => vec![
                    bare,
                    SlashCommandInvocation::SandboxReadRoot(SandboxReadRootArgs {
                        path: "/tmp/test-dir".to_string(),
                    }),
                ],
                SlashCommand::Review => vec![
                    bare,
                    SlashCommandInvocation::Review(ReviewArgs {
                        instructions: placeholder_text_arg("[Image #1]"),
                    }),
                ],
                SlashCommand::Rename => vec![
                    bare,
                    SlashCommandInvocation::Rename(RenameArgs {
                        title: SlashTextArg::new("ship it".to_string(), Vec::new()),
                    }),
                ],
                SlashCommand::Plan => vec![
                    bare,
                    SlashCommandInvocation::Plan(PlanArgs {
                        prompt: SlashTextArg::new("investigate flaky test".to_string(), Vec::new()),
                    }),
                ],
                SlashCommand::Statusline => vec![
                    bare,
                    SlashCommandInvocation::Statusline(StatuslineArgs {
                        items: vec![StatusLineItem::ModelName, StatusLineItem::CurrentDir],
                    }),
                ],
                SlashCommand::Feedback => vec![
                    bare,
                    SlashCommandInvocation::Feedback(FeedbackArgs {
                        category: FeedbackCategory::BadResult,
                    }),
                    SlashCommandInvocation::Feedback(FeedbackArgs {
                        category: FeedbackCategory::GoodResult,
                    }),
                    SlashCommandInvocation::Feedback(FeedbackArgs {
                        category: FeedbackCategory::Bug,
                    }),
                    SlashCommandInvocation::Feedback(FeedbackArgs {
                        category: FeedbackCategory::SafetyCheck,
                    }),
                    SlashCommandInvocation::Feedback(FeedbackArgs {
                        category: FeedbackCategory::Other,
                    }),
                ],
                SlashCommand::Model
                | SlashCommand::Approvals
                | SlashCommand::Permissions
                | SlashCommand::ElevateSandbox
                | SlashCommand::Experimental
                | SlashCommand::Skills
                | SlashCommand::New
                | SlashCommand::Resume
                | SlashCommand::Fork
                | SlashCommand::Init
                | SlashCommand::Compact
                | SlashCommand::Collab
                | SlashCommand::Agent
                | SlashCommand::Diff
                | SlashCommand::Copy
                | SlashCommand::Mention
                | SlashCommand::Status
                | SlashCommand::DebugConfig
                | SlashCommand::Title
                | SlashCommand::Theme
                | SlashCommand::Mcp
                | SlashCommand::Apps
                | SlashCommand::Plugins
                | SlashCommand::Logout
                | SlashCommand::Quit
                | SlashCommand::Exit
                | SlashCommand::Rollout
                | SlashCommand::Ps
                | SlashCommand::Stop
                | SlashCommand::Clear
                | SlashCommand::Personality
                | SlashCommand::Realtime
                | SlashCommand::Settings
                | SlashCommand::TestApproval
                | SlashCommand::MultiAgents
                | SlashCommand::MemoryDrop
                | SlashCommand::MemoryUpdate => vec![bare],
            }
        }

        fn roundtrip_strategy(self) -> BoxedStrategy<SlashCommandInvocation> {
            let bare = Just(SlashCommandInvocation::Bare(self));
            match self {
                SlashCommand::Fast => prop_oneof![
                    bare,
                    select(vec![
                        FastSlashCommandArgs::On,
                        FastSlashCommandArgs::Off,
                        FastSlashCommandArgs::Status,
                    ])
                    .prop_map(|mode| SlashCommandInvocation::Fast(FastArgs { mode })),
                ]
                .boxed(),
                SlashCommand::SandboxReadRoot => prop_oneof![
                    bare,
                    string_arg_strategy().prop_map(|path| {
                        SlashCommandInvocation::SandboxReadRoot(SandboxReadRootArgs { path })
                    }),
                ]
                .boxed(),
                SlashCommand::Review => prop_oneof![
                    bare,
                    text_arg_strategy().prop_map(|instructions| {
                        SlashCommandInvocation::Review(ReviewArgs { instructions })
                    }),
                ]
                .boxed(),
                SlashCommand::Rename => prop_oneof![
                    bare,
                    plain_text_arg_strategy()
                        .prop_map(|title| { SlashCommandInvocation::Rename(RenameArgs { title }) }),
                ]
                .boxed(),
                SlashCommand::Plan => prop_oneof![
                    bare,
                    text_arg_strategy()
                        .prop_map(|prompt| { SlashCommandInvocation::Plan(PlanArgs { prompt }) }),
                ]
                .boxed(),
                SlashCommand::Statusline => {
                    let items = vec![
                        StatusLineItem::ModelName,
                        StatusLineItem::ModelWithReasoning,
                        StatusLineItem::CurrentDir,
                        StatusLineItem::ProjectRoot,
                        StatusLineItem::GitBranch,
                        StatusLineItem::ContextRemaining,
                        StatusLineItem::ContextUsed,
                        StatusLineItem::FiveHourLimit,
                        StatusLineItem::WeeklyLimit,
                        StatusLineItem::CodexVersion,
                        StatusLineItem::ContextWindowSize,
                        StatusLineItem::UsedTokens,
                        StatusLineItem::TotalInputTokens,
                        StatusLineItem::TotalOutputTokens,
                        StatusLineItem::SessionId,
                        StatusLineItem::FastMode,
                    ];
                    prop_oneof![
                        bare,
                        proptest::collection::vec(select(items), 1..5).prop_map(|items| {
                            SlashCommandInvocation::Statusline(StatuslineArgs { items })
                        }),
                    ]
                    .boxed()
                }
                SlashCommand::Feedback => prop_oneof![
                    bare,
                    select(vec![
                        FeedbackCategory::BadResult,
                        FeedbackCategory::GoodResult,
                        FeedbackCategory::Bug,
                        FeedbackCategory::SafetyCheck,
                        FeedbackCategory::Other,
                    ])
                    .prop_map(|category| {
                        SlashCommandInvocation::Feedback(FeedbackArgs { category })
                    }),
                ]
                .boxed(),
                SlashCommand::Model
                | SlashCommand::Approvals
                | SlashCommand::Permissions
                | SlashCommand::ElevateSandbox
                | SlashCommand::Experimental
                | SlashCommand::Skills
                | SlashCommand::New
                | SlashCommand::Resume
                | SlashCommand::Fork
                | SlashCommand::Init
                | SlashCommand::Compact
                | SlashCommand::Collab
                | SlashCommand::Agent
                | SlashCommand::Diff
                | SlashCommand::Copy
                | SlashCommand::Mention
                | SlashCommand::Status
                | SlashCommand::DebugConfig
                | SlashCommand::Title
                | SlashCommand::Theme
                | SlashCommand::Mcp
                | SlashCommand::Apps
                | SlashCommand::Plugins
                | SlashCommand::Logout
                | SlashCommand::Quit
                | SlashCommand::Exit
                | SlashCommand::Rollout
                | SlashCommand::Ps
                | SlashCommand::Stop
                | SlashCommand::Clear
                | SlashCommand::Personality
                | SlashCommand::Realtime
                | SlashCommand::Settings
                | SlashCommand::TestApproval
                | SlashCommand::MultiAgents
                | SlashCommand::MemoryDrop
                | SlashCommand::MemoryUpdate => bare.boxed(),
            }
        }
    }

    #[test]
    fn all_registered_commands_roundtrip_from_serialized_text() {
        for spec in SLASH_COMMAND_SPECS {
            for invocation in spec.command.roundtrip_test_invocations() {
                let (args, text_elements) = serialized_args(&invocation);
                assert_eq!(
                    spec.command.parse_invocation(&args, &text_elements),
                    Ok(invocation.clone()),
                    "roundtrip failed for /{} with serialized {:?}",
                    spec.command.command(),
                    invocation.serialize().text
                );
            }
        }
    }

    #[test]
    fn all_registered_commands_proptest_roundtrip_from_serialized_text() {
        for spec in SLASH_COMMAND_SPECS {
            let command = spec.command;
            let mut runner = TestRunner::new(ProptestConfig {
                cases: 24,
                failure_persistence: None,
                ..ProptestConfig::default()
            });
            runner
                .run(&command.roundtrip_strategy(), |invocation| {
                    let serialized = invocation.serialize();
                    let (args, text_elements) = serialized_args(&invocation);
                    let reparsed =
                        command
                            .parse_invocation(&args, &text_elements)
                            .map_err(|err| {
                                TestCaseError::fail(format!(
                                    "roundtrip parse failed for /{} from {:?}: {err:?}",
                                    command.command(),
                                    serialized.text
                                ))
                            })?;
                    prop_assert_eq!(reparsed, invocation);
                    Ok(())
                })
                .unwrap_or_else(|err| {
                    panic!(
                        "property roundtrip failed for /{}: {err}",
                        command.command()
                    )
                });
        }
    }

    #[test]
    fn approvals_alias_is_hidden_from_command_popup() {
        assert!(SlashCommand::Approvals.hide_in_command_popup());
    }

    #[test]
    fn clean_alias_parses_to_stop_command() {
        assert_eq!(SlashCommand::from_str("clean"), Ok(SlashCommand::Stop));
    }

    #[test]
    fn stop_command_is_canonical_name() {
        assert_eq!(SlashCommand::Stop.command(), "stop");
    }

    #[test]
    fn fast_usage_lists_bare_and_arg_forms() {
        assert_eq!(
            SlashCommand::Fast.usage_lines(),
            ["/fast", "/fast [on|off|status]"]
        );
    }

    #[test]
    fn clear_usage_is_bare_only() {
        assert_eq!(SlashCommand::Clear.usage_lines(), ["/clear"]);
    }

    #[test]
    fn review_bare_form_is_marked_as_ui_driven() {
        assert_eq!(
            SlashCommand::Review.parse_invocation("", &[]),
            Ok(SlashCommandInvocation::Bare(SlashCommand::Review))
        );
        assert_eq!(
            SlashCommand::Review.spec().bare_behavior,
            SlashCommandBareBehavior::OpensUi
        );
    }

    #[test]
    fn fast_accepts_nonempty_inline_args() {
        assert_eq!(
            SlashCommand::Fast.parse_invocation("status", &[]),
            Ok(SlashCommandInvocation::Fast(FastArgs {
                mode: FastSlashCommandArgs::Status,
            }))
        );
    }

    #[test]
    fn feedback_accepts_category_arg() {
        assert_eq!(
            SlashCommand::Feedback.parse_invocation("bug", &[]),
            Ok(SlashCommandInvocation::Feedback(FeedbackArgs {
                category: FeedbackCategory::Bug,
            }))
        );
    }

    #[test]
    fn statusline_accepts_variadic_item_list() {
        assert_eq!(
            SlashCommand::Statusline.parse_invocation("model-name current-dir", &[]),
            Ok(SlashCommandInvocation::Statusline(StatuslineArgs {
                items: vec![StatusLineItem::ModelName, StatusLineItem::CurrentDir],
            }))
        );
    }

    #[test]
    fn sandbox_read_root_accepts_named_path_arg() {
        let invocation = SlashCommand::SandboxReadRoot
            .parse_invocation("--path='/tmp/test dir'", &[])
            .unwrap();

        assert_eq!(
            invocation,
            SlashCommandInvocation::SandboxReadRoot(SandboxReadRootArgs {
                path: "/tmp/test dir".to_string(),
            })
        );
        assert_eq!(
            invocation.serialize().text,
            "/sandbox-add-read-dir '/tmp/test dir'"
        );
    }

    #[test]
    fn review_preserves_placeholder_elements() {
        let placeholder = "[Image #1]".to_string();
        let text_elements = vec![codex_protocol::user_input::TextElement::new(
            (0..placeholder.len()).into(),
            Some(placeholder.clone()),
        )];

        assert_eq!(
            SlashCommand::Review.parse_invocation(&placeholder, &text_elements),
            Ok(SlashCommandInvocation::Review(ReviewArgs {
                instructions: SlashTextArg::new(placeholder, text_elements),
            }))
        );
    }

    #[test]
    fn clear_rejects_unexpected_inline_args() {
        assert_eq!(
            SlashCommand::Clear
                .parse_invocation("now", &[])
                .unwrap_err()
                .message(),
            "'/clear' does not accept inline arguments. Usage: /clear"
        );
    }

    #[test]
    fn plan_serialization_preserves_placeholder_ranges() {
        let placeholder = "[Image #1]".to_string();
        let invocation = SlashCommandInvocation::Plan(PlanArgs {
            prompt: SlashTextArg::new(
                format!("review {placeholder}"),
                vec![codex_protocol::user_input::TextElement::new(
                    (7..18).into(),
                    Some(placeholder.clone()),
                )],
            ),
        });

        assert_eq!(
            invocation.serialize(),
            SlashSerializedText {
                text: format!("/plan review {placeholder}"),
                text_elements: vec![codex_protocol::user_input::TextElement::new(
                    (13..24).into(),
                    Some(placeholder),
                )],
            }
        );
    }
}
