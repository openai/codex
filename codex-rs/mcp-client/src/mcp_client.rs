//! A minimal async client for the Model Context Protocol (MCP).
//!
//! The client is intentionally lightweight – it is only capable of:
//!   1. Spawning a subprocess that launches a conforming MCP server that
//!      communicates over stdio.
//!   2. Sending MCP requests and pairing them with their corresponding
//!      responses.
//!   3. Offering a convenience helper for the common `tools/list` request.
//!
//! The crate hides all JSON‐RPC framing details behind a typed API. Users
//! interact with the [`ModelContextProtocolRequest`] trait from `mcp-types` to
//! issue requests and receive strongly-typed results.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use mcp_types::CallToolRequest;
use mcp_types::CallToolRequestParams;
use mcp_types::InitializeRequest;
use mcp_types::InitializeRequestParams;
use mcp_types::InitializedNotification;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCMessage;
use mcp_types::JSONRPCRequest;
use mcp_types::JSONRPCResponse;
use mcp_types::ListToolsRequest;
use mcp_types::ListToolsRequestParams;
use mcp_types::ListToolsResult;
use mcp_types::ModelContextProtocolNotification;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::{AsyncBufRead, AsyncWrite};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use mcp_types::JSONRPCNotification;
use tokio::sync::oneshot;
use tokio::time;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Capacity of the bounded channels used for transporting messages between the
/// client API and the IO tasks.
const CHANNEL_CAPACITY: usize = 128;

/// Internal representation of a pending request sender.
type PendingSender = oneshot::Sender<JSONRPCMessage>;

impl McpClient {
    /// Create a new MCP client from any AsyncBufRead/AsyncWrite pair.
    /// Does not manage a child process (child field will be None).
    pub fn with_streams<R, W>(reader: R, writer: W) -> Self
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
        let pending: Arc<Mutex<HashMap<i64, PendingSender>>> = Arc::new(Mutex::new(HashMap::new()));
        let (notification_tx, notification_rx) = mpsc::channel::<JSONRPCNotification>(CHANNEL_CAPACITY);

        // Writer task: send messages to the writer
        let _writer_handle = {
            let mut writer = writer;
            tokio::spawn(async move {
                while let Some(msg) = outgoing_rx.recv().await {
                    match serde_json::to_string(&msg) {
                        Ok(json) => {
                            debug!("MCP message to server: {json}");
                            if writer.write_all(json.as_bytes()).await.is_err() {
                                error!("failed to write message to writer");
                                break;
                            }
                            if writer.write_all(b"\n").await.is_err() {
                                error!("failed to write newline to writer");
                                break;
                            }
                            if writer.flush().await.is_err() {
                                error!("failed to flush writer");
                                break;
                            }
                        }
                        Err(e) => error!("failed to serialize JSONRPCMessage: {e}"),
                    }
                }
            })
        };

        // Reader task: receive messages from the reader and dispatch
        let _reader_handle = {
            let pending = pending.clone();
            let mut lines = reader.lines();
            let notification_tx = notification_tx.clone();
            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    match serde_json::from_str::<JSONRPCMessage>(&line) {
                        Ok(JSONRPCMessage::Response(resp)) => {
                            Self::dispatch_response(resp, &pending).await;
                        }
                        Ok(JSONRPCMessage::Error(err)) => {
                            Self::dispatch_error(err, &pending).await;
                        }
                        Ok(JSONRPCMessage::Notification(note)) => {
                            let _ = notification_tx.send(note).await;
                        }
                        _ => {}
                    }
                }
            })
        };

        McpClient {
            child: None,
            outgoing_tx,
            pending,
            id_counter: AtomicI64::new(1),
            notification_rx,
        }
    }
}

/// A running MCP client instance.
pub struct McpClient {
    /// Optional child process handle; `None` for in-memory/test clients.
    #[allow(dead_code)]
    child: Option<tokio::process::Child>,

    /// Channel for sending JSON-RPC messages *to* the background writer task.
    outgoing_tx: mpsc::Sender<JSONRPCMessage>,

    /// Map of `request.id -> oneshot::Sender` used to dispatch responses back
    /// to the originating caller.
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,

    /// Monotonically increasing counter used to generate request IDs.
    id_counter: AtomicI64,
    /// Receives server-initiated notifications.
    notification_rx: mpsc::Receiver<JSONRPCNotification>,
}

impl McpClient {
    /// Spawn the given command and establish an MCP session over its STDIO.
    /// Caller is responsible for sending the `initialize` request. See
    /// [`initialize`](Self::initialize) for details.
    pub async fn new_stdio_client(
        program: String,
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
    ) -> std::io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .env_clear()
            .envs(create_env_for_mcp_server(env))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            // As noted in the `kill_on_drop` documentation, the Tokio runtime makes
            // a "best effort" to reap-after-exit to avoid zombie processes, but it
            // is not a guarantee.
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("failed to capture child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("failed to capture child stdout"))?;

        // Set up client streams over stdio
        let mut client = Self::with_streams(BufReader::new(stdout), stdin);
        // Retain child process handle
        client.child = Some(child);
        Ok(client)
    }

    /// Send an arbitrary MCP request and await the typed result.
    ///
    /// If `timeout` is `None` the call waits indefinitely. If `Some(duration)`
    /// is supplied and no response is received within the given period, a
    /// timeout error is returned.
    pub async fn send_request<R>(
        &self,
        params: R::Params,
        timeout: Option<Duration>,
    ) -> Result<R::Result>
    where
        R: ModelContextProtocolRequest,
        R::Params: Serialize,
        R::Result: DeserializeOwned,
    {
        // Create a new unique ID.
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        let request_id = RequestId::Integer(id);

        // Serialize params -> JSON. For many request types `Params` is
        // `Option<T>` and `None` should be encoded as *absence* of the field.
        let params_json = serde_json::to_value(&params)?;
        let params_field = if params_json.is_null() {
            None
        } else {
            Some(params_json)
        };

        let jsonrpc_request = JSONRPCRequest {
            id: request_id.clone(),
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: R::METHOD.to_string(),
            params: params_field,
        };

        let message = JSONRPCMessage::Request(jsonrpc_request);

        // oneshot channel for the response.
        let (tx, rx) = oneshot::channel();

        // Register in pending map *before* sending the message so a race where
        // the response arrives immediately cannot be lost.
        {
            let mut guard = self.pending.lock().await;
            guard.insert(id, tx);
        }

        // Send to writer task.
        if self.outgoing_tx.send(message).await.is_err() {
            return Err(anyhow!(
                "failed to send message to writer task - channel closed"
            ));
        }

        // Await the response, optionally bounded by a timeout.
        let msg = match timeout {
            Some(duration) => {
                match time::timeout(duration, rx).await {
                    Ok(Ok(msg)) => msg,
                    Ok(Err(_)) => {
                        // Channel closed without a reply – remove the pending entry.
                        let mut guard = self.pending.lock().await;
                        guard.remove(&id);
                        return Err(anyhow!(
                            "response channel closed before a reply was received"
                        ));
                    }
                    Err(_) => {
                        // Timed out. Remove the pending entry so we don't leak.
                        let mut guard = self.pending.lock().await;
                        guard.remove(&id);
                        return Err(anyhow!("request timed out"));
                    }
                }
            }
            None => rx
                .await
                .map_err(|_| anyhow!("response channel closed before a reply was received"))?,
        };

        match msg {
            JSONRPCMessage::Response(JSONRPCResponse { result, .. }) => {
                let typed: R::Result = serde_json::from_value(result)?;
                Ok(typed)
            }
            JSONRPCMessage::Error(err) => Err(anyhow!(format!(
                "server returned JSON-RPC error: code = {}, message = {}",
                err.error.code, err.error.message
            ))),
            other => Err(anyhow!(format!(
                "unexpected message variant received in reply path: {:?}",
                other
            ))),
        }
    }

    pub async fn send_notification<N>(&self, params: N::Params) -> Result<()>
    where
        N: ModelContextProtocolNotification,
        N::Params: Serialize,
    {
        // Serialize params -> JSON. For many request types `Params` is
        // `Option<T>` and `None` should be encoded as *absence* of the field.
        let params_json = serde_json::to_value(&params)?;
        let params_field = if params_json.is_null() {
            None
        } else {
            Some(params_json)
        };

        let method = N::METHOD.to_string();
        let jsonrpc_notification = JSONRPCNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.clone(),
            params: params_field,
        };

        let notification = JSONRPCMessage::Notification(jsonrpc_notification);
        self.outgoing_tx
            .send(notification)
            .await
            .with_context(|| format!("failed to send notification `{method}` to writer task"))
    }

    /// Negotiates the initialization with the MCP server. Sends an `initialize`
    /// request with the specified `initialize_params` and then the
    /// `notifications/initialized` notification once the response has been
    /// received. Returns the response to the `initialize` request.
    pub async fn initialize(
        &self,
        initialize_params: InitializeRequestParams,
        initialize_notification_params: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::InitializeResult> {
        let response = self
            .send_request::<InitializeRequest>(initialize_params, timeout)
            .await?;
        self.send_notification::<InitializedNotification>(initialize_notification_params)
            .await?;
        Ok(response)
    }

    /// Convenience wrapper around `tools/list`.
    pub async fn list_tools(
        &self,
        params: Option<ListToolsRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsResult> {
        self.send_request::<ListToolsRequest>(params, timeout).await
    }

    /// Convenience wrapper around `tools/call`.
    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::CallToolResult> {
        let params = CallToolRequestParams { name, arguments };
        debug!("MCP tool call: {params:?}");
        self.send_request::<CallToolRequest>(params, timeout).await
    }
    /// Receive the next notification sent by the server.
    /// Returns None if the notification channel is closed.
    pub async fn next_notification(&mut self) -> Option<JSONRPCNotification> {
        self.notification_rx.recv().await
    }

    /// Internal helper: route a JSON-RPC *response* object to the pending map.
    async fn dispatch_response(
        resp: JSONRPCResponse,
        pending: &Arc<Mutex<HashMap<i64, PendingSender>>>,
    ) {
        let id = match resp.id {
            RequestId::Integer(i) => i,
            RequestId::String(_) => {
                // We only ever generate integer IDs. Receiving a string here
                // means we will not find a matching entry in `pending`.
                error!("response with string ID - no matching pending request");
                return;
            }
        };

        if let Some(tx) = pending.lock().await.remove(&id) {
            // Ignore send errors – the receiver might have been dropped.
            let _ = tx.send(JSONRPCMessage::Response(resp));
        } else {
            warn!(id, "no pending request found for response");
        }
    }

    /// Internal helper: route a JSON-RPC *error* object to the pending map.
    async fn dispatch_error(
        err: mcp_types::JSONRPCError,
        pending: &Arc<Mutex<HashMap<i64, PendingSender>>>,
    ) {
        let id = match err.id {
            RequestId::Integer(i) => i,
            RequestId::String(_) => return, // see comment above
        };

        if let Some(tx) = pending.lock().await.remove(&id) {
            let _ = tx.send(JSONRPCMessage::Error(err));
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Even though we have already tagged this process with
        // `kill_on_drop(true)` above, this extra check has the benefit of
        // forcing the process to be reaped immediately if it has already exited
        // instead of waiting for the Tokio runtime to reap it later.
        if let Some(child) = &mut self.child {
            let _ = child.try_wait();
        }
    }
}

/// Environment variables that are always included when spawning a new MCP
/// server.
#[rustfmt::skip]
#[cfg(unix)]
const DEFAULT_ENV_VARS: &[&str] = &[
    // https://modelcontextprotocol.io/docs/tools/debugging#environment-variables
    // states:
    //
    // > MCP servers inherit only a subset of environment variables automatically,
    // > like `USER`, `HOME`, and `PATH`.
    //
    // But it does not fully enumerate the list. Empirically, when spawning a
    // an MCP server via Claude Desktop on macOS, it reports the following
    // environment variables:
    "HOME",
    "LOGNAME",
    "PATH",
    "SHELL",
    "USER",
    "__CF_USER_TEXT_ENCODING",

    // Additional environment variables Codex chooses to include by default:
    "LANG",
    "LC_ALL",
    "TERM",
    "TMPDIR",
    "TZ",
];

#[cfg(windows)]
const DEFAULT_ENV_VARS: &[&str] = &[
    // TODO: More research is necessary to curate this list.
    "PATH",
    "PATHEXT",
    "USERNAME",
    "USERDOMAIN",
    "USERPROFILE",
    "TEMP",
    "TMP",
];

/// `extra_env` comes from the config for an entry in `mcp_servers` in
/// `config.toml`.
fn create_env_for_mcp_server(
    extra_env: Option<HashMap<String, String>>,
) -> HashMap<String, String> {
    DEFAULT_ENV_VARS
        .iter()
        .filter_map(|var| match std::env::var(var) {
            Ok(value) => Some((var.to_string(), value)),
            Err(_) => None,
        })
        .chain(extra_env.unwrap_or_default())
        .collect::<HashMap<_, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_env_for_mcp_server() {
        let env_var = "USER";
        let env_var_existing_value = std::env::var(env_var).unwrap_or_default();
        let env_var_new_value = format!("{env_var_existing_value}-extra");
        let extra_env = HashMap::from([(env_var.to_owned(), env_var_new_value.clone())]);
        let mcp_server_env = create_env_for_mcp_server(Some(extra_env));
        assert!(mcp_server_env.contains_key("PATH"));
        assert_eq!(Some(&env_var_new_value), mcp_server_env.get(env_var));
    }
}
