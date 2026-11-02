//! Runner-owned HTTP lifecycle over the existing in-memory board and host interfaces.

use crate::MessageBoardMcpServer;
use crate::serve_http;
use axum::http::HeaderValue;
use codex_message_board::AgentMessageBoard;
use codex_message_board::InMemoryAgentMessageBoard;
use codex_message_board::MessageBoardHost;
use codex_message_board::MessageBoardTool;
use codex_protocol::SessionId;
use rmcp::ErrorData;
use rmcp::transport::StreamableHttpServerConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

type BoardBinding = (Arc<dyn AgentMessageBoard>, Arc<dyn MessageBoardHost>);
type Boards = Arc<RwLock<HashMap<SessionId, BoardBinding>>>;

/// One runner-owned endpoint. Dropping it cancels HTTP and MCP sessions.
/// Credentials are deliberately excluded from Debug and startup output.
pub struct LocalServer {
    url: String,
    authorization: HeaderValue,
    boards: Boards,
    cancellation: CancellationToken,
    task: Option<JoinHandle<anyhow::Result<()>>>,
}

impl LocalServer {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn authorization(&self) -> &HeaderValue {
        &self.authorization
    }

    /// Registers a tree before its first tool call. The runner supplies authoritative
    /// membership and clocks; the service never registers or spawns agents itself.
    pub fn add_board(
        &self,
        identity: SessionId,
        host: Arc<dyn MessageBoardHost>,
    ) -> anyhow::Result<()> {
        let mut boards = self
            .boards
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        anyhow::ensure!(
            !boards.contains_key(&identity),
            "board session already registered"
        );
        boards.insert(
            identity,
            (
                Arc::new(InMemoryAgentMessageBoard::new(identity, host.clone())),
                host,
            ),
        );
        Ok(())
    }

    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        if let Some(task) = self.task.take() {
            task.await??;
        }
        Ok(())
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// Binds a loopback endpoint for a local run. Register hosts before tool calls.
/// All connections for a tree share one board; the last owner releases its state.
pub async fn launch_in_memory() -> anyhow::Result<LocalServer> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let url = format!("http://{}/mcp", listener.local_addr()?);
    let token = uuid::Uuid::new_v4().to_string();
    let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))?;
    authorization.set_sensitive(true);
    let boards = Boards::default();
    let binding = boards.clone();
    let cancellation = CancellationToken::new();
    let task = tokio::spawn(serve_http(
        listener,
        token,
        StreamableHttpServerConfig::default().with_cancellation_token(cancellation.clone()),
        move |caller, session_id| {
            let board = binding
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&session_id)
                .cloned();
            async move {
                let (board, host) = board.ok_or_else(|| {
                    ErrorData::invalid_params("Unknown board session", /*data*/ None)
                })?;
                let path = host.agent_path(caller).await.map_err(|error| {
                    ErrorData::invalid_params(error.to_string(), /*data*/ None)
                })?;
                Ok(MessageBoardMcpServer::new(
                    &MessageBoardTool::DISCUSSION,
                    board,
                    caller,
                    path,
                ))
            }
        },
    ));
    Ok(LocalServer {
        url,
        authorization,
        boards,
        cancellation,
        task: Some(task),
    })
}
