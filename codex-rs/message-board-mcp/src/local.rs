//! Runner-owned HTTP lifecycle over the existing in-memory board and host interfaces.
//! Agent-specific bindings share tree state while retaining each caller's host.
//! Timestamps use the MCP backend's system clock, independently of the agent clock.

use crate::MessageBoardMcpServer;
use crate::serve_http;
use axum::http::HeaderValue;
use chrono::DateTime;
use chrono::Utc;
use codex_message_board::AgentMessageBoard;
use codex_message_board::InMemoryAgentMessageBoard;
use codex_message_board::InMemoryMessageBoards;
use codex_message_board::MessageBoardHost;
use codex_message_board::MessageBoardTool;
use codex_message_board::NotificationDelivery;
use codex_message_board::PostPreview;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use rmcp::ErrorData;
use rmcp::transport::StreamableHttpServerConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

type BoardBinding = (Arc<dyn AgentMessageBoard>, Arc<dyn MessageBoardHost>);
type Boards = Arc<RwLock<HashMap<(SessionId, Option<ThreadId>), BoardBinding>>>;

struct BackendClockHost(Arc<dyn MessageBoardHost>);

impl MessageBoardHost for BackendClockHost {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        self.0.agent_path(caller)
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        self.0.resolve_agent(path)
    }

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        Box::pin(async move {
            self.0.agent_path(caller).await?;
            Ok(Utc::now())
        })
    }

    fn notify(
        &self,
        recipient: ThreadId,
        post: PostPreview,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        self.0.notify(recipient, post)
    }
}

/// One runner-owned endpoint. Dropping it initiates HTTP and MCP session cleanup.
/// Call [`Self::shutdown`] to wait until that cleanup has finished.
/// Credentials are deliberately excluded from Debug and startup output.
pub struct LocalServer {
    url: String,
    authorization: HeaderValue,
    boards: Boards,
    states: InMemoryMessageBoards,
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
    /// membership; the service never registers or spawns agents itself.
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
            !boards.keys().any(|(tree, _)| *tree == identity),
            "board session already registered"
        );
        let host: Arc<dyn MessageBoardHost> = Arc::new(BackendClockHost(host));
        boards.insert(
            (identity, None),
            (
                Arc::new(InMemoryAgentMessageBoard::new(identity, host.clone())),
                host,
            ),
        );
        Ok(())
    }

    /// Binds one caller's host to shared tree state, instead of using a tree-wide host.
    pub async fn bind_agent(
        &self,
        identity: SessionId,
        caller: ThreadId,
        host: Arc<dyn MessageBoardHost>,
    ) -> anyhow::Result<()> {
        let host: Arc<dyn MessageBoardHost> = Arc::new(BackendClockHost(host));
        let board = self.states.open(identity, host.clone()).await;
        let mut boards = self
            .boards
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        anyhow::ensure!(
            !boards.contains_key(&(identity, None)),
            "board session already has a tree-wide host"
        );
        boards.insert((identity, Some(caller)), (Arc::new(board), host));
        Ok(())
    }

    /// Closes sessions, cancels pending bindings, and drains dispatched board operations.
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
            let board = {
                let boards = binding
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                boards
                    .get(&(session_id, Some(caller)))
                    .or_else(|| boards.get(&(session_id, None)))
                    .cloned()
            };
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
        states: InMemoryMessageBoards::default(),
        cancellation,
        task: Some(task),
    })
}
