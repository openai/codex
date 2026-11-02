//! Installs agent message-board tools over the shared board interface.
//! Storage, backend behavior and tool implementations live in `codex-message-board`.

mod extension;
mod mcp;

pub use extension::install;
pub use mcp::install_mcp;

/// Storage selected by the agent consumer for opening, setup and permanent deletion.
pub fn local_board_options() -> codex_message_board::LocalBoardOptions {
    codex_message_board::LocalBoardOptions {
        storage_namespace: "agent_message_board".into(),
    }
}
