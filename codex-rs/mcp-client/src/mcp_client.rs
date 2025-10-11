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
use std::ffi::OsString;
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
use mcp_types::JSONRPCNotification;
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
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
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

/// A running MCP client instance.
pub struct McpClient {
    /// Retain this child process until the client is dropped. The Tokio runtime
    /// will make a "best effort" to reap the process after it exits, but it is
    /// not a guarantee. See the `kill_on_drop` documentation for details.
    #[allow(dead_code)]
    child: tokio::process::Child,

    /// Channel for sending JSON-RPC messages *to* the background writer task.
    outgoing_tx: mpsc::Sender<JSONRPCMessage>,

    /// Map of `request.id -> oneshot::Sender` used to dispatch responses back
    /// to the originating caller.
    pending: Arc<Mutex<HashMap<i64, PendingSender>>>,

    /// Monotonically increasing counter used to generate request IDs.
    id_counter: AtomicI64,
}

impl McpClient {
    /// Spawn the given command and establish an MCP session over its STDIO.
    /// Caller is responsible for sending the `initialize` request. See
    /// [`initialize`](Self::initialize) for details.
    pub async fn new_stdio_client(
        program: OsString,
        args: Vec<OsString>,
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

        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
        let pending: Arc<Mutex<HashMap<i64, PendingSender>>> = Arc::new(Mutex::new(HashMap::new()));

        // Spawn writer task. It listens on the `outgoing_rx` channel and
        // writes messages to the child's STDIN.
        let writer_handle = {
            let mut stdin = stdin;
            tokio::spawn(async move {
                while let Some(msg) = outgoing_rx.recv().await {
                    match serde_json::to_string(&msg) {
                        Ok(json) => {
                            debug!("MCP message to server: {json}");
                            if stdin.write_all(json.as_bytes()).await.is_err() {
                                error!("failed to write message to child stdin");
                                break;
                            }
                            if stdin.write_all(b"\n").await.is_err() {
                                error!("failed to write newline to child stdin");
                                break;
                            }
                            // No explicit flush needed on a pipe; write_all is sufficient.
                        }
                        Err(e) => error!("failed to serialize JSONRPCMessage: {e}"),
                    }
                }
            })
        };

        // Spawn reader task. It reads line-delimited JSON from the child's
        // STDOUT and dispatches responses to the pending map.
        let reader_handle = {
            let pending = pending.clone();
            let mut lines = BufReader::new(stdout).lines();

            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!("MCP message from server: {line}");
                    match serde_json::from_str::<JSONRPCMessage>(&line) {
                        Ok(JSONRPCMessage::Response(resp)) => {
                            Self::dispatch_response(resp, &pending).await;
                        }
                        Ok(JSONRPCMessage::Error(err)) => {
                            Self::dispatch_error(err, &pending).await;
                        }
                        Ok(JSONRPCMessage::Notification(JSONRPCNotification { .. })) => {
                            // For now we only log server-initiated notifications.
                            info!("<- notification: {}", line);
                        }
                        Ok(other) => {
                            // Batch responses and requests are currently not
                            // expected from the server – log and ignore.
                            info!("<- unhandled message: {:?}", other);
                        }
                        Err(e) => {
                            error!("failed to deserialize JSONRPCMessage: {e}; line = {}", line)
                        }
                    }
                }
            })
        };

        // We intentionally *detach* the tasks. They will keep running in the
        // background as long as their respective resources (channels/stdin/
        // stdout) are alive. Dropping `McpClient` cancels the tasks due to
        // dropped resources.
        let _ = (writer_handle, reader_handle);

        Ok(Self {
            child,
            outgoing_tx,
            pending,
            id_counter: AtomicI64::new(1),
        })
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
                "unexpected message variant received in reply path: {other:?}"
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
        timeout: Option<Duration>,
    ) -> Result<mcp_types::InitializeResult> {
        let response = self
            .send_request::<InitializeRequest>(initialize_params, timeout)
            .await?;
        self.send_notification::<InitializedNotification>(None)
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

        let tx_opt = {
            let mut guard = pending.lock().await;
            guard.remove(&id)
        };
        if let Some(tx) = tx_opt {
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

        let tx_opt = {
            let mut guard = pending.lock().await;
            guard.remove(&id)
        };
        if let Some(tx) = tx_opt {
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
        let _ = self.child.try_wait();
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
    "APPDATA",    // Required for npm configuration and cache storage
    "SYSTEMROOT", // Required for DNS resolution and network operations
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

    // Windows-specific tests for validating critical environment variables required by npm/npx operations.
    // These tests empirically verify which environment variables must be passed to MCP servers on Windows.
    //
    // To run with detailed output:
    // cargo test -p codex-mcp-client --lib tests::windows_env_tests -- --ignored --nocapture --test-threads=1
    #[cfg(windows)]
    mod windows_env_tests {
        use super::*;
        use std::path::PathBuf;
        use std::time::Duration;
        use tokio::process::Command;
        use tokio::time;

        // Test configuration constants
        const TEST_TIMEOUT: Duration = Duration::from_secs(10);
        const TEST_PACKAGE: &str = "@openai/codex";

        // Command names
        const NPX: &str = "npx";
        const NPM: &str = "npm";

        // Critical environment variables required for npm/npx operations on Windows
        // These have been validated through empirical testing
        const CRITICAL_ENV_VARS: &[&str] = &[
            "PATH",       // Required for executable discovery
            "APPDATA",    // Required for npm configuration and cache storage
            "SYSTEMROOT", // Required for DNS resolution and network operations
        ];

        /// Result type for test command execution
        type CommandResult = Result<bool, CommandError>;

        /// Errors that can occur during command execution
        #[derive(Debug, Clone, PartialEq)]
        enum CommandError {
            SpawnFailed,
            Timeout,
            WaitFailed,
            NotFound(String),
        }

        impl std::fmt::Display for CommandError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::SpawnFailed => write!(f, "Failed to spawn command"),
                    Self::Timeout => write!(f, "Command execution timed out"),
                    Self::WaitFailed => write!(f, "Failed to wait for process completion"),
                    Self::NotFound(cmd) => write!(f, "{} command not found", cmd),
                }
            }
        }

        /// Test command builder for executing npm/npx operations with controlled environments
        struct TestCommand {
            program: PathBuf,
            args: Vec<String>,
            env: HashMap<String, String>,
            timeout: Duration,
        }

        impl TestCommand {
            /// Creates a command to check npm package version via npx
            fn npm_view_via_npx(program: PathBuf) -> Self {
                Self {
                    program,
                    args: vec![
                        "npm".to_string(),
                        "view".to_string(),
                        TEST_PACKAGE.to_string(),
                        "version".to_string(),
                    ],
                    env: create_env_for_mcp_server(None),
                    timeout: TEST_TIMEOUT,
                }
            }

            /// Creates a command to check npm package version directly
            fn npm_view(program: PathBuf) -> Self {
                Self {
                    program,
                    args: vec![
                        "view".to_string(),
                        TEST_PACKAGE.to_string(),
                        "version".to_string(),
                    ],
                    env: create_env_for_mcp_server(None),
                    timeout: TEST_TIMEOUT,
                }
            }

            /// Creates a command to check npx version (non-network operation)
            fn npx_version(program: PathBuf) -> Self {
                Self {
                    program,
                    args: vec!["--version".to_string()],
                    env: create_env_for_mcp_server(None),
                    timeout: TEST_TIMEOUT,
                }
            }

            /// Removes a specific environment variable from the command's environment
            fn without_env(mut self, key: &str) -> Self {
                self.env.remove(key);
                self
            }

            /// Executes the command and returns success status or error
            async fn execute(&self) -> CommandResult {
                let mut child = Command::new(&self.program)
                    .args(&self.args)
                    .env_clear()
                    .envs(&self.env)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|_| CommandError::SpawnFailed)?;

                match time::timeout(self.timeout, child.wait()).await {
                    Ok(Ok(status)) => Ok(status.success()),
                    Ok(Err(_)) => Err(CommandError::WaitFailed),
                    Err(_) => {
                        // Timeout occurred - ensure child process is killed
                        if let Err(e) = child.kill().await {
                            eprintln!("Warning: Failed to kill timed-out process: {}", e);
                        }
                        // Wait for process to actually terminate after kill
                        let _ = child.wait().await;
                        Err(CommandError::Timeout)
                    }
                }
            }
        }

        /// Utility to find and validate npm command availability
        fn find_npm() -> Result<PathBuf, CommandError> {
            which::which(NPM).map_err(|_| CommandError::NotFound(NPM.to_string()))
        }

        /// Utility to find and validate npx command availability
        fn find_npx() -> Result<PathBuf, CommandError> {
            which::which(NPX).map_err(|_| CommandError::NotFound(NPX.to_string()))
        }

        /// Helper macro to skip test if command is not available
        macro_rules! require_command {
            ($finder:expr) => {
                match $finder {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Skipping test: {}", e);
                        return;
                    }
                }
            };
        }

        // ========== SYSTEMROOT Environment Variable Tests ==========

        #[tokio::test]
        #[ignore = "Requires npx installation"]
        async fn npx_version_works_without_systemroot() {
            // Validates that non-network operations don't require SYSTEMROOT
            let npx_path = require_command!(find_npx());

            let result = TestCommand::npx_version(npx_path)
                .without_env("SYSTEMROOT")
                .execute()
                .await;

            assert!(
                matches!(result, Ok(true)),
                "npx --version should succeed without SYSTEMROOT for non-network operations"
            );
        }

        #[tokio::test]
        #[ignore = "Requires npx and network access"]
        async fn npx_network_operation_requires_systemroot() {
            // Validates that network operations fail without SYSTEMROOT (DNS resolution fails)
            let npx_path = require_command!(find_npx());

            let result = TestCommand::npm_view_via_npx(npx_path)
                .without_env("SYSTEMROOT")
                .execute()
                .await;

            match result {
                Ok(false) | Err(CommandError::Timeout) => {
                    // Expected: command fails or hangs without SYSTEMROOT
                }
                Ok(true) => {
                    panic!("Network operation unexpectedly succeeded without SYSTEMROOT");
                }
                Err(e) => {
                    panic!("Unexpected error: {}", e);
                }
            }
        }

        // ========== APPDATA Environment Variable Tests ==========

        #[tokio::test]
        #[ignore = "Requires npx installation"]
        async fn npx_version_works_without_appdata() {
            // Validates that simple npx operations don't require APPDATA
            let npx_path = require_command!(find_npx());

            let result = TestCommand::npx_version(npx_path)
                .without_env("APPDATA")
                .execute()
                .await;

            assert!(
                matches!(result, Ok(true)),
                "npx --version should succeed without APPDATA for simple operations"
            );
        }

        #[tokio::test]
        #[ignore = "Requires npx and network access"]
        async fn npx_network_operation_requires_appdata() {
            // Validates that npx network operations fail without APPDATA (can't access npm cache)
            let npx_path = require_command!(find_npx());

            let result = TestCommand::npm_view_via_npx(npx_path)
                .without_env("APPDATA")
                .execute()
                .await;

            assert!(
                matches!(result, Ok(false)),
                "npx network operations should fail without APPDATA"
            );
        }

        #[tokio::test]
        #[ignore = "Requires npm and network access"]
        async fn npm_network_operation_works_without_appdata() {
            // Validates that npm can fall back to default cache location when APPDATA is missing
            let npm_path = require_command!(find_npm());

            let result = TestCommand::npm_view(npm_path)
                .without_env("APPDATA")
                .execute()
                .await;

            assert!(
                matches!(result, Ok(true)),
                "npm should handle missing APPDATA by using fallback cache location"
            );
        }

        // ========== Baseline and Validation Tests ==========

        #[tokio::test]
        #[ignore = "Requires npx and network access"]
        async fn npx_works_with_complete_environment() {
            // Baseline test: validates that npx works correctly with all required env vars
            let npx_path = require_command!(find_npx());

            let cmd = TestCommand::npm_view_via_npx(npx_path);

            // Verify critical variables are present
            for var in CRITICAL_ENV_VARS {
                assert!(
                    cmd.env.contains_key(*var),
                    "Critical environment variable {} is missing",
                    var
                );
            }

            let result = cmd.execute().await;
            assert!(
                matches!(result, Ok(true)),
                "npx should work with complete environment"
            );
        }

        // ========== Comprehensive Validation Test ==========
        //
        // To run this test with detailed output showing the validation process:
        // cargo test -p codex-mcp-client --lib tests::windows_env_tests::validate_critical_environment_variables -- --ignored --nocapture
        //
        // This test empirically validates which Windows environment variables are
        // critical for npm/npx operations by systematically removing each variable
        // and testing if the operation still succeeds.

        #[tokio::test]
        #[ignore = "Requires npx and network access"]
        async fn validate_critical_environment_variables() {
            let npx_path = require_command!(find_npx());
            // Base environment with all default vars that MCP servers receive
            let base_env = create_env_for_mcp_server(None);

            // Filter to only test variables that exist in the actual environment
            let mut vars_to_test: Vec<String> = base_env
                .keys()
                .filter(|var| std::env::var(var).is_ok())
                .cloned()
                .collect();

            // Sort alphabetically for consistent output
            vars_to_test.sort();

            println!("\n=== CRITICAL ENVIRONMENT VARIABLES VALIDATION ===");
            println!(
                "Testing {} environment variables for npm/npx criticality...",
                vars_to_test.len()
            );
            println!("Each variable will be removed and npm/npx tested without it.\n");

            // Create tasks for all tests
            let mut tasks = Vec::new();
            for var_name in vars_to_test.iter() {
                let var_name = var_name.clone();
                let npx_path = npx_path.clone();

                let task = tokio::spawn(async move {
                    eprintln!("  Testing without {}...", var_name);

                    let result = TestCommand::npm_view_via_npx(npx_path)
                        .without_env(&var_name)
                        .execute()
                        .await;

                    let is_critical = matches!(result, Ok(false) | Err(CommandError::Timeout));

                    (var_name, is_critical, result)
                });

                tasks.push(task);
            }

            // Wait for all tasks to complete and collect results
            let mut test_results = Vec::new();
            for task in tasks {
                match task.await {
                    Ok(result) => test_results.push(result),
                    Err(e) => {
                        panic!("Task failed: {}", e);
                    }
                }
            }

            // Sort results for consistent output
            test_results.sort_by(|a, b| a.0.cmp(&b.0));

            // Print detailed test results
            println!("\n=== Test Results ===");
            let max_var_len = test_results
                .iter()
                .map(|(name, _, _)| name.len())
                .max()
                .unwrap_or(10);
            let header_width = max_var_len.max(20);

            println!("{:<width$} | Status", "Variable", width = header_width);
            println!("{}", "-".repeat(header_width + 40));

            for (name, is_critical, _) in &test_results {
                if *is_critical {
                    println!(
                        "{:<width$} | CRITICAL for npm/npx operations",
                        name,
                        width = header_width
                    );
                } else {
                    println!("{:<width$} | Not critical", name, width = header_width);
                }
            }

            // Extract critical variables from test results
            let mut found_critical: Vec<&str> = test_results
                .iter()
                .filter_map(|(name, is_critical, _)| {
                    if *is_critical {
                        Some(name.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            found_critical.sort();

            let mut expected_critical: Vec<&str> = CRITICAL_ENV_VARS.to_vec();
            expected_critical.sort();

            // Print summary for better visibility
            println!("\n=== TEST RESULTS SUMMARY ===");
            println!("Critical variables found:    {:?}", found_critical);
            println!("Critical variables expected: {:?}", expected_critical);

            // Provide detailed error message if mismatch occurs
            if found_critical != expected_critical {
                let found_set: std::collections::HashSet<&str> =
                    found_critical.iter().copied().collect();
                let expected_set: std::collections::HashSet<&str> =
                    expected_critical.iter().copied().collect();
                let missing = expected_set.difference(&found_set).collect::<Vec<_>>();
                let unexpected = found_set.difference(&expected_set).collect::<Vec<_>>();

                panic!(
                    "Critical environment variables validation failed!\n\
                Expected but not found critical: {:?}\n\
                Found critical but not expected: {:?}\n\
                Full test results:\n{}",
                    missing,
                    unexpected,
                    test_results
                        .iter()
                        .map(|(name, critical, result)| {
                            format!(
                                "  {} = {} (result: {:?})",
                                name,
                                if *critical {
                                    "CRITICAL"
                                } else {
                                    "not critical"
                                },
                                result
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }

            // Verify all critical vars are included in the base environment
            for var in CRITICAL_ENV_VARS {
                assert!(
                    base_env.contains_key(*var),
                    "Critical variable {} must be included in default environment",
                    var
                );
            }

            println!("\n=== VALIDATION SUCCESSFUL ===");
            println!(
                "\n{} critical environment variables confirmed:",
                CRITICAL_ENV_VARS.len()
            );
            println!("  - PATH: Required for executable discovery");
            println!("  - APPDATA: Required for npm configuration and cache storage");
            println!("  - SYSTEMROOT: Required for DNS resolution and network operations");
        }
    }
}
