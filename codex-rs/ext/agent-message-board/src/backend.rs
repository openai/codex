//! Opens the host-selected board backend using the existing lifecycle factory.
//! Hosts supply membership, clocks and notification delivery through `MessageBoardHost`.
//! Installation opens no storage; each runtime binds its own host at thread startup.

use crate::extension;
use crate::local_board_options;
use crate::mcp;
use codex_agent_message_board_client::AccessToken;
use codex_agent_message_board_client::RemoteAgentMessageBoard;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartInput;
use codex_features::RemoteMessageBoardConfigToml;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_message_board::AgentMessageBoard;
use codex_message_board::InMemoryMessageBoards;
use codex_message_board::LocalAgentMessageBoard;
use codex_message_board::LocalBoardConnection;
use codex_message_board::MessageBoardHost;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_state::SqliteConfig;
use std::sync::Arc;

/// A backend selected from the host's effective configuration at thread startup.
pub enum BoardBackend {
    Mcp,
    Connected(Arc<LocalBoardConnection>),
    Remote {
        config: RemoteMessageBoardConfigToml,
        http: HttpClientFactory,
    },
    InMemory,
    Local(SqliteConfig),
}

/// Installs board backends and tools without depending on the host's runtime types.
/// The host selects a backend and supplies its capabilities, or returns `None`
/// when disabled. Remote hosts are retained in the thread store for the existing
/// turn-bound notification receiver; this does not change delivery semantics.
pub fn install<C: Sync + 'static, H: MessageBoardHost + Clone + 'static>(
    registry: &mut ExtensionRegistryBuilder<C>,
    namespace_description: &'static str,
    tool_namespace: impl Fn(&C) -> Option<String> + Send + Sync + 'static,
    host: impl Fn(&ThreadStartInput<'_, C>, SessionId, ThreadId) -> Option<(BoardBackend, H)>
    + Send
    + Sync
    + 'static,
) {
    let in_memory_boards = Arc::new(InMemoryMessageBoards::default());
    let mcp_boards = mcp::install_mcp(registry);
    extension::install(
        registry,
        namespace_description,
        tool_namespace,
        move |input, tree, caller| {
            let Some((backend, host)) = host(input, tree, caller) else {
                return Box::pin(async { Ok(None) });
            };
            let in_memory_boards = Arc::clone(&in_memory_boards);
            let mcp_boards = Arc::clone(&mcp_boards);
            let host = Arc::new(host);
            Box::pin(async move {
                let board: Arc<dyn AgentMessageBoard> = match backend {
                    BoardBackend::Mcp => {
                        mcp_boards
                            .open(input.thread_store, tree, caller, host)
                            .await?;
                        return Ok(None);
                    }
                    BoardBackend::Connected(connection) => Arc::new(connection.open(host).await?),
                    BoardBackend::Remote { config, http } => {
                        let token = match config.bearer_token_env_var {
                            Some(name) => std::env::var(name).map_err(|_| CodexErr::InvalidRequest(
                                "message-board credential environment variable is missing or invalid".into(),
                            ))?,
                            None => config.bearer_token.ok_or_else(|| CodexErr::InvalidRequest(
                                "remote message board requires a credential".into(),
                            ))?.into_inner(),
                        };
                        let token = AccessToken::new(token)
                            .map_err(|err| CodexErr::InvalidRequest(err.to_string()))?;
                        let http = http
                            .build_client(&config.url, ClientRouteClass::Api)
                            .map_err(|err| CodexErr::Io(std::io::Error::other(err)))?;
                        input.thread_store.insert(host.as_ref().clone());
                        let board = RemoteAgentMessageBoard::new(http, &config.url, tree, token)
                            .map_err(|err| CodexErr::InvalidRequest(err.to_string()))?
                            .with_clock(move |caller| {
                                let host = host.clone();
                                Box::pin(async move { host.current_time(caller).await })
                            });
                        input.thread_store.get_or_init(|| board)
                    }
                    BoardBackend::InMemory => Arc::new(in_memory_boards.open(tree, host).await),
                    BoardBackend::Local(sqlite) => Arc::new(
                        LocalAgentMessageBoard::open(&sqlite, &local_board_options(), tree, host)
                            .await?,
                    ),
                };
                Ok(Some(board))
            })
        },
    );
}
