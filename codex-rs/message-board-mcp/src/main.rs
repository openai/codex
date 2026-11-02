//! Detached HTTP entry point; endpoint configuration and process lifetime belong to the deployer.

mod persisted;

use clap::Parser;
use codex_message_board::LocalAgentMessageBoard;
use codex_message_board::LocalBoardOptions;
use codex_message_board::MessageBoardHost;
use codex_message_board::MessageBoardTool;
use codex_message_board_mcp::MessageBoardMcpServer;
use codex_state::SqliteConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use rmcp::ErrorData;
use rmcp::transport::StreamableHttpServerConfig;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Parser)]
struct Args {
    /// Absolute directory containing Codex's state and message-board databases.
    #[arg(long)]
    sqlite_home: std::path::PathBuf,
    #[arg(long, default_value = "127.0.0.1:7777")]
    listen: std::net::SocketAddr,
    /// Additional allowed HTTP Host names or host:port authorities. Loopback remains allowed.
    #[arg(long = "allowed-host")]
    allowed_hosts: Vec<String>,
    /// Name of the environment variable containing the trusted-host bearer token.
    #[arg(long, default_value = "CODEX_MESSAGE_BOARD_TOKEN")]
    bearer_token_env: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let token = std::env::var(&args.bearer_token_env).map_err(|_| {
        anyhow::anyhow!("MCP bearer token environment variable is missing or invalid")
    })?;
    let sqlite = SqliteConfig::from_sqlite_home_with_nfs_enforcement(
        AbsolutePathBuf::try_from(args.sqlite_home)?,
        /*enforce_delete_on_nfs*/ false,
    );
    let registry = sqlite
        .open_read_only_pool(&sqlite.state_db_path(), /*busy_timeout*/ None)
        .await?;
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    println!(
        "{}",
        serde_json::json!({"url": format!("http://{}/mcp", listener.local_addr()?)})
    );
    let cancellation = tokio_util::sync::CancellationToken::new();
    let mut http =
        StreamableHttpServerConfig::default().with_cancellation_token(cancellation.clone());
    http.allowed_hosts.extend(args.allowed_hosts);
    // All sessions share one SQLite file. Retain a board between calls so the
    // weak pool registry reuses its connections instead of initializing again.
    let pool_owner = Arc::new(Mutex::new(None));
    let serving =
        codex_message_board_mcp::serve_http(listener, token, http, move |caller, session_id| {
            let pool_owner = Arc::clone(&pool_owner);
            let sqlite = sqlite.clone();
            let host = Arc::new(persisted::PersistedHost {
                registry: registry.clone(),
                root: session_id.into(),
            });
            async move {
                let path = host.agent_path(caller).await.map_err(|error| {
                    ErrorData::invalid_params(error.to_string(), /*data*/ None)
                })?;
                let board = LocalAgentMessageBoard::open(
                    &sqlite,
                    &LocalBoardOptions::default(),
                    session_id,
                    host,
                )
                .await
                .map_err(|error| {
                    ErrorData::internal_error(error.to_string(), /*data*/ None)
                })?;
                *pool_owner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(board.clone());
                Ok(MessageBoardMcpServer::new(
                    &MessageBoardTool::DISCUSSION,
                    Arc::new(board),
                    caller,
                    path,
                ))
            }
        });
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => result,
        _ = tokio::signal::ctrl_c() => {
            cancellation.cancel();
            serving.await
        }
    }
}
