// Slash command definitions for Codex TUI

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommand {
    Clear,
    ClearHistory,
    Compact,
    History,
    Help,
    Model,
    Approval,
    Bug,
    Diff,
}

pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub command: SlashCommand,
}

pub static COMMANDS: &[CommandInfo] = &[
    CommandInfo { name: "/clear", description: "Clear conversation history and free up context", command: SlashCommand::Clear },
    CommandInfo { name: "/clearhistory", description: "Clear command history", command: SlashCommand::ClearHistory },
    CommandInfo { name: "/compact", description: "Clear conversation history but keep a summary in context. Optional: /compact [instructions for summarization]", command: SlashCommand::Compact },
    CommandInfo { name: "/history", description: "Open command history", command: SlashCommand::History },
    CommandInfo { name: "/help", description: "Show list of commands", command: SlashCommand::Help },
    CommandInfo { name: "/model", description: "Open model selection panel", command: SlashCommand::Model },
    CommandInfo { name: "/approval", description: "Open approval mode selection panel", command: SlashCommand::Approval },
    CommandInfo { name: "/bug", description: "Generate a prefilled GitHub issue URL with session log", command: SlashCommand::Bug },
    CommandInfo { name: "/diff", description: "Show git diff of the working directory (or applied patches if not in git)", command: SlashCommand::Diff },
];