//! Guardian's durable evidence storage, owned by the main thread.
//! Shared boards provide persistence and access checks. This crate starts no background work
//! and exposes no model tools; ingestion and review integration follow separately.

use codex_message_board::LocalBoard;
use codex_message_board::LocalBoardOptions;
use codex_protocol::SessionId;
use codex_state::SqliteConfig;

/// Shared by setup, opening and cleanup, including when Guardian is disabled.
pub fn local_board_options() -> LocalBoardOptions {
    LocalBoardOptions {
        storage_namespace: "guardian".into(),
    }
}

/// Selects the board for the owning main thread, never the temporary reviewer or housekeeper.
/// Hosts use a read-write connection; reviewers use a read-only connection, which still
/// permits their own explicit subscriptions. Cleanup uses the same namespace and root identity.
pub fn local_board(sqlite: SqliteConfig, root: SessionId) -> LocalBoard {
    LocalBoard::new(sqlite, root).with_options(local_board_options())
}
