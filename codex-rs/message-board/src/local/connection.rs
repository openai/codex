//! Host-supplied connections to one local board. Access is fixed per runtime,
//! independent of the board's storage or the agent's tree identity.

use super::LocalAgentMessageBoard;
use super::options::LocalBoardOptions;
use crate::MessageBoardHost;
use crate::PermissionDenied;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

/// Shared local storage from which a host grants individual runtime connections.
#[derive(Clone)]
pub struct LocalBoard {
    sqlite: SqliteConfig,
    identity: SessionId,
    options: LocalBoardOptions,
}

impl LocalBoard {
    pub fn new(sqlite: SqliteConfig, identity: SessionId) -> Self {
        Self {
            sqlite,
            identity,
            options: LocalBoardOptions::default(),
        }
    }

    pub fn with_options(mut self, options: LocalBoardOptions) -> Self {
        self.options = options;
        self
    }

    pub fn read_only(&self) -> LocalBoardConnection {
        LocalBoardConnection {
            board: self.clone(),
            access: Access::ReadOnly,
            database: OnceLock::new(),
        }
    }

    pub fn read_write(&self) -> LocalBoardConnection {
        LocalBoardConnection {
            board: self.clone(),
            access: Access::ReadWrite,
            database: OnceLock::new(),
        }
    }
}

/// Insert into `StartThreadOptions::thread_extension_init` before creating Codex.
/// Hosts must supply it again for a new runtime, including cold resume or a fork.
/// The board extension must be installed, including in an isolated session's registry.
#[derive(Clone)]
pub struct LocalBoardConnection {
    board: LocalBoard,
    access: Access,
    // Capture the opened database, independently of caller unload or symlink changes.
    database: OnceLock<PathBuf>,
}

impl LocalBoardConnection {
    pub fn identity(&self) -> SessionId {
        self.board.identity
    }

    /// Compare the canonical database paths captured when the connections were opened.
    pub fn same_board(&self, other: &Self) -> bool {
        self.identity() == other.identity()
            && self
                .database
                .get()
                .zip(other.database.get())
                .is_some_and(|(database, other)| database == other)
    }

    /// Independent runtimes have no common agent tree. Use their unique IDs as paths.
    pub fn caller_path(&self, caller: ThreadId) -> Result<AgentPath> {
        AgentPath::root()
            .join(&caller.to_string().replace('-', ""))
            .map_err(CodexErr::InvalidRequest)
    }

    pub async fn open(&self, host: Arc<dyn MessageBoardHost>) -> Result<LocalAgentMessageBoard> {
        let mut board = LocalAgentMessageBoard::open(
            &self.board.sqlite,
            &self.board.options,
            self.identity(),
            host,
        )
        .await?;
        // Pool initialization canonicalizes the directory before opening SQLite.
        let options = board.pool.connect_options();
        let database = options.get_filename();
        if self.database.get_or_init(|| database.to_path_buf()) != database {
            return Err(CodexErr::InvalidRequest(
                "board database changed; create a new board connection".into(),
            ));
        }
        board.access = Some(self.access);
        Ok(board)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Access {
    ReadOnly,
    ReadWrite,
}

impl Access {
    pub(super) fn require_write(self, board: SessionId, action: &str) -> Result<()> {
        match self {
            Self::ReadWrite => Ok(()),
            Self::ReadOnly => Err(PermissionDenied {
                message: "This board connection is read-only.".into(),
                action: Some(action.into()),
                resource: Some(format!("board:{board}")),
            }
            .into()),
        }
    }
}
