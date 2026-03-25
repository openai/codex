//! Sigils for tool/plugin mentions in plaintext (shared across Codex crates).

/// Default plaintext sigil for tools.
pub const TOOL_MENTION_SIGIL: char = '$';

/// Plugins use `@` in linked plaintext outside TUI.
pub const PLUGIN_TEXT_MENTION_SIGIL: char = '@';
