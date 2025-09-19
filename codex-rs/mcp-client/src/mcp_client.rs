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
        let envs = create_env_for_mcp_server(env);

        // Resolve program to executable path (platform-specific)
        let executable = program_resolver::resolve(program, &envs)?;

        let mut child = Command::new(executable)
            .args(args)
            .env_clear()
            .envs(envs)
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

/// Platform-specific program resolution for MCP server execution.
///
/// Resolves executable paths differently based on the operating system:
/// - **Windows**: Uses `which` crate to find executables including scripts (.cmd, .bat)
/// - **Unix**: Returns the program unchanged (native PATH resolution handles it)
mod program_resolver {
    use super::*;

    /// Resolves a program to its executable path for the current platform.
    ///
    /// On Windows, `Command::new()` cannot execute scripts (e.g., `.cmd`, `.bat`)
    /// directly without their extension. This function uses the `which` crate to
    /// search the `PATH` and find the full path to the executable, including
    /// any necessary script extensions defined in `PATHEXT`.
    ///
    /// On Unix, the kernel handles script execution natively (via shebangs), so
    /// this function is a no-op and returns the program name unchanged.
    #[cfg(windows)]
    pub fn resolve(program: OsString, env: &HashMap<String, String>) -> std::io::Result<OsString> {
        use std::env;

        // Get current directory for relative path resolution
        let cwd = env::current_dir().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to get current directory: {}", e),
            )
        })?;

        // Extract PATH from environment for search locations
        let search_path = env.get("PATH");

        // Attempt resolution via which crate
        match which::which_in(&program, search_path, &cwd) {
            Ok(resolved) => {
                debug!("Resolved {:?} to {:?}", program, resolved);
                Ok(resolved.into_os_string())
            }
            Err(e) => {
                debug!(
                    "Failed to resolve {:?}: {}. Using original path",
                    program, e
                );
                // Fallback to original program - let Command::new() handle the error
                Ok(program)
            }
        }
    }

    /// Unix systems handle PATH resolution natively.
    ///
    /// The OS can execute scripts directly, so no resolution needed.
    #[cfg(unix)]
    pub fn resolve(program: OsString, _env: &HashMap<String, String>) -> std::io::Result<OsString> {
        Ok(program)
    }
}

#[cfg(test)]
mod tests {
    use super::program_resolver;
    use super::*;
    use anyhow::Result;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

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

    /// Unix: Verifies the OS handles script execution without file extensions.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_unix_executes_script_without_extension() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        let mut cmd = Command::new(&env.program_name);
        cmd.envs(&env.mcp_env);

        let output = cmd.output().await;
        assert!(output.is_ok(), "Unix should execute scripts directly");
        Ok(())
    }

    /// Windows: Verifies scripts fail to execute without the proper extension.
    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_fails_without_extension() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        let mut cmd = Command::new(&env.program_name);
        cmd.envs(&env.mcp_env);

        let output = cmd.output().await;
        assert!(
            output.is_err(),
            "Windows requires .cmd/.bat extension for direct execution"
        );
        Ok(())
    }

    /// Windows: Verifies scripts with an explicit extension execute correctly.
    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_succeeds_with_extension() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        // Append the `.cmd` extension to the program name
        let program_with_ext = format!("{}.cmd", env.program_name);
        let mut cmd = Command::new(&program_with_ext);
        cmd.envs(&env.mcp_env);

        let output = cmd.output().await;
        assert!(
            output.is_ok(),
            "Windows should execute scripts when the extension is provided"
        );
        Ok(())
    }

    /// Verifies program resolution enables successful execution on all platforms.
    #[tokio::test]
    async fn test_resolved_program_executes_successfully() -> Result<()> {
        let env = TestExecutableEnv::new()?;
        let program = OsString::from(&env.program_name);

        // Apply platform-specific resolution
        let resolved = program_resolver::resolve(program, &env.mcp_env)?;

        // Verify resolved path executes successfully
        let mut cmd = Command::new(resolved);
        cmd.envs(&env.mcp_env);
        let output = cmd.output().await;

        assert!(
            output.is_ok(),
            "Resolved program should execute successfully"
        );
        Ok(())
    }

    // Test fixture for creating temporary executables in a controlled environment.
    struct TestExecutableEnv {
        // Held to prevent the temporary directory from being deleted.
        _temp_dir: TempDir,
        program_name: String,
        mcp_env: HashMap<String, String>,
    }

    impl TestExecutableEnv {
        const TEST_PROGRAM: &'static str = "test_mcp_server";

        fn new() -> Result<Self> {
            let temp_dir = TempDir::new()?;
            let dir_path = temp_dir.path();

            Self::create_executable(dir_path)?;

            // Build a clean environment with the temp dir in the PATH.
            let mut extra_env = HashMap::new();
            extra_env.insert("PATH".to_string(), Self::build_path(dir_path));

            #[cfg(windows)]
            extra_env.insert("PATHEXT".to_string(), Self::ensure_cmd_extension());

            let mcp_env = create_env_for_mcp_server(Some(extra_env));

            Ok(Self {
                _temp_dir: temp_dir,
                program_name: Self::TEST_PROGRAM.to_string(),
                mcp_env,
            })
        }

        /// Creates a simple, platform-specific executable script.
        fn create_executable(dir: &Path) -> Result<()> {
            #[cfg(windows)]
            {
                let file = dir.join(format!("{}.cmd", Self::TEST_PROGRAM));
                fs::write(&file, "@echo off\nexit 0")?;
            }

            #[cfg(unix)]
            {
                let file = dir.join(Self::TEST_PROGRAM);
                fs::write(&file, "#!/bin/sh\nexit 0")?;
                Self::set_executable(&file)?;
            }

            Ok(())
        }

        #[cfg(unix)]
        fn set_executable(path: &Path) -> Result<()> {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
            Ok(())
        }

        /// Prepends the given directory to the system's PATH variable.
        fn build_path(dir: &Path) -> String {
            let current = std::env::var("PATH").unwrap_or_default();
            let sep = if cfg!(windows) { ";" } else { ":" };
            format!("{}{sep}{current}", dir.to_string_lossy())
        }

        /// Ensures `.CMD` is in the `PATHEXT` variable on Windows for script discovery.
        #[cfg(windows)]
        fn ensure_cmd_extension() -> String {
            let current = std::env::var("PATHEXT").unwrap_or_default();
            if current.to_uppercase().contains(".CMD") {
                current
            } else {
                format!(".CMD;{current}")
            }
        }
    }
}
