// Command classification rules - Pure data definitions
//
// This module contains static rule definitions that map command patterns
// to categories. No logic lives here - this is just data.

use crate::approval::rules::predicate_rules;

/// Categories describe what a command CAN do based on its observable properties.
/// These are descriptive, not prescriptive - the engine makes policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandCategory {
    /// Command observes filesystem state (ls, cat, find without -exec)
    ReadsFilesystem,

    /// Command observes VCS state (git status, git log)
    ReadsVcs,

    /// Command modifies files (touch, mkdir, echo >)
    ModifiesFilesystem,

    /// Command modifies VCS state (git commit, git add)
    ModifiesVcs,

    /// Command can delete data (rm -rf, git reset --hard)
    DeletesData,

    /// Command not recognized by any rule
    Unrecognized,
}

/// Defines how to match a command against a pattern
#[derive(Debug)]
pub enum CommandMatcher {
    /// Always matches (e.g., "ls" is always ObservesFilesystem)
    Always,

    /// Matches if subcommand is in the list (e.g., git [status|log])
    WithSubcommands(&'static [&'static str]),

    /// Matches if none of the forbidden args are present (e.g., find without -exec)
    WithoutForbiddenArgs(&'static [&'static str]),

    /// Custom predicate for complex matching
    Custom(fn(&[String]) -> bool),
}

/// A single rule mapping a tool + matcher to a category
#[derive(Debug)]
pub struct CommandRule {
    pub tool: &'static str,
    pub matcher: CommandMatcher,
    pub category: CommandCategory,
    pub has_subcommand: Option<bool>,
}

impl CommandRule {
    pub const fn new(
        tool: &'static str,
        matcher: CommandMatcher,
        category: CommandCategory,
    ) -> CommandRule {
        CommandRule {
            tool,
            matcher,
            category,
            has_subcommand: None,
        }
    }

    pub const fn with_subcommand(
        tool: &'static str,
        matcher: CommandMatcher,
        category: CommandCategory,
    ) -> CommandRule {
        CommandRule {
            tool,
            matcher,
            category,
            has_subcommand: Some(true),
        }
    }
}

/// Static rule definitions - all pattern matching logic is here as data
pub static COMMAND_RULES: &[CommandRule] = &[
    // Filesystem observation tools
    CommandRule::new(
        "ls",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "cat",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "cd",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "echo",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "head",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "tail",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "pwd",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "grep",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "true",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "false",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "wc",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "which",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "nl",
        CommandMatcher::Always,
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "find",
        CommandMatcher::WithoutForbiddenArgs(&[
            "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fls", "-fprint", "-fprint0",
            "-fprintf",
        ]),
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "rg",
        CommandMatcher::WithoutForbiddenArgs(&["--pre", "--hostname-bin", "--search-zip", "-z"]),
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::new(
        "sed",
        CommandMatcher::Custom(predicate_rules::is_sed_permitted),
        CommandCategory::ReadsFilesystem,
    ),
    CommandRule::with_subcommand(
        "cargo",
        CommandMatcher::WithSubcommands(&["check"]),
        CommandCategory::ReadsFilesystem,
    ),
    // Data deletion
    CommandRule::new(
        "rm",
        CommandMatcher::WithoutForbiddenArgs(&["-f", "--force", "-r", "-R", "--recursive"]),
        CommandCategory::Unrecognized,
    ),
    CommandRule::new("rm", CommandMatcher::Always, CommandCategory::DeletesData),
];
