//! Guardian's durable evidence storage, owned by the main thread.
//! Reviewer tools search a fixed board view and deliver the originals routed by the housekeeper.
//! The host owns review scheduling and the lifetime of each view.
//! Hosts invoke permanent-deletion cleanup only when Guardian V3 is enabled.

use codex_message_board::LocalBoard;
use codex_message_board::LocalBoardOptions;
use codex_protocol::SessionId;
use codex_state::SqliteConfig;

mod delivery;
mod evidence;
mod housekeeper;
mod retrieval_tools;
mod sampling;
mod selection;
mod subscription_tools;
pub use delivery::DeliveryPlan;
pub use delivery::DeliveryState;
pub use evidence::EvidenceCursor;
pub use evidence::EvidenceId;
pub use evidence::EvidenceSnapshot;
pub use evidence::OriginalEvidence;
pub use housekeeper::HOUSEKEEPER_INSTRUCTIONS;
pub use housekeeper::HOUSEKEEPER_TIMEOUT;
pub use housekeeper::Housekeeper;
pub use retrieval_tools::RetrievalBinding;
pub use retrieval_tools::install_retrieval_tools;
pub use sampling::HousekeeperResult;
pub use sampling::OperationResult;
pub use selection::CHANNEL_SELECTION_INSTRUCTIONS;
pub use selection::ReviewerSubscriptions;

const GUARDIAN_BOARD_NAMESPACE: &str = "guardian";

/// Storage selected by the Guardian consumer for setup, opening and cleanup.
pub fn local_board_options() -> LocalBoardOptions {
    LocalBoardOptions {
        storage_namespace: GUARDIAN_BOARD_NAMESPACE.into(),
    }
}

/// Selects the board for the owning main thread, never the temporary reviewer or housekeeper.
/// Hosts use a read-write connection; reviewers use a read-only connection, which still
/// permits their own explicit subscriptions. Cleanup uses the same namespace and root identity.
pub fn local_board(sqlite: SqliteConfig, root: SessionId) -> LocalBoard {
    LocalBoard::new(sqlite, root).with_options(local_board_options())
}

/// Permanently deletes evidence boards owned by these main threads.
/// Hosts invoke this only when Guardian V3 is enabled.
pub async fn delete_local_boards(
    sqlite: &SqliteConfig,
    roots: &[SessionId],
) -> codex_protocol::error::Result<()> {
    codex_message_board::LocalAgentMessageBoard::delete_boards(
        sqlite,
        &local_board_options(),
        roots,
    )
    .await
}
