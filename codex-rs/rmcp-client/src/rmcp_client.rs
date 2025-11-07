use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use futures::FutureExt;
use mcp_types::CallToolRequestParams;
use mcp_types::CallToolResult;
use mcp_types::InitializeRequestParams;
use mcp_types::InitializeResult;
use mcp_types::ListResourceTemplatesRequestParams;
use mcp_types::ListResourceTemplatesResult;
use mcp_types::ListResourcesRequestParams;
use mcp_types::ListResourcesResult;
use mcp_types::ListToolsRequestParams;
use mcp_types::ListToolsResult;
use mcp_types::ReadResourceRequestParams;
use mcp_types::ReadResourceResult;
use reqwest::header::HeaderMap;
use rmcp::model::CallToolRequestParam;
use rmcp::model::InitializeRequestParam;
use rmcp::model::PaginatedRequestParam;
use rmcp::model::ReadResourceRequestParam;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::service::{self};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time;
use tracing::info;
use tracing::warn;

use crate::load_oauth_tokens;
use crate::logging_client_handler::LoggingClientHandler;
use crate::oauth::OAuthCredentialsStoreMode;
use crate::oauth::OAuthPersistor;
use crate::oauth::StoredOAuthTokens;
use crate::utils::apply_default_headers;
use crate::utils::build_default_headers;
use crate::utils::convert_call_tool_result;
use crate::utils::convert_to_mcp;
use crate::utils::convert_to_rmcp;
use crate::utils::create_env_for_mcp_server;
use crate::utils::run_with_timeout;

enum PendingTransport {
    ChildProcess(TokioChildProcess),
    StreamableHttp {
        transport: StreamableHttpClientTransport<reqwest::Client>,
    },
    StreamableHttpWithOAuth {
        transport: StreamableHttpClientTransport<AuthClient<reqwest::Client>>,
        oauth_persistor: OAuthPersistor,
    },
}

enum ClientState {
    Connecting {
        transport: Option<PendingTransport>,
    },
    Ready {
        service: Arc<RunningService<RoleClient, LoggingClientHandler>>,
        oauth: Option<OAuthPersistor>,
    },
}

/// MCP client implemented on top of the official `rmcp` SDK.
/// https://github.com/modelcontextprotocol/rust-sdk
pub struct RmcpClient {
    state: Mutex<ClientState>,
}

impl RmcpClient {
    pub async fn new_stdio_client(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<String, String>>,
        env_vars: &[String],
        cwd: Option<PathBuf>,
    ) -> io::Result<Self> {
        let program_name = program.to_string_lossy().into_owned();

        // Build environment for program resolution and subprocess
        let envs = create_env_for_mcp_server(env, env_vars);

        // Resolve program to executable path (platform-specific)
        let resolved_program = program_resolver::resolve(program, &envs)?;

        let mut command = Command::new(resolved_program);
        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env_clear()
            .envs(envs)
            .args(&args);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }

        let (transport, stderr) = TokioChildProcess::builder(command)
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                loop {
                    match reader.next_line().await {
                        Ok(Some(line)) => {
                            info!("MCP server stderr ({program_name}): {line}");
                        }
                        Ok(None) => break,
                        Err(error) => {
                            warn!("Failed to read MCP server stderr ({program_name}): {error}");
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(PendingTransport::ChildProcess(transport)),
            }),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new_streamable_http_client(
        server_name: &str,
        url: &str,
        bearer_token: Option<String>,
        http_headers: Option<HashMap<String, String>>,
        env_http_headers: Option<HashMap<String, String>>,
        store_mode: OAuthCredentialsStoreMode,
    ) -> Result<Self> {
        let default_headers = build_default_headers(http_headers, env_http_headers)?;

        let initial_oauth_tokens = match bearer_token {
            Some(_) => None,
            None => match load_oauth_tokens(server_name, url, store_mode) {
                Ok(tokens) => tokens,
                Err(err) => {
                    warn!("failed to read tokens for server `{server_name}`: {err}");
                    None
                }
            },
        };

        let transport = if let Some(initial_tokens) = initial_oauth_tokens.clone() {
            let (transport, oauth_persistor) = create_oauth_transport_and_runtime(
                server_name,
                url,
                initial_tokens,
                store_mode,
                default_headers.clone(),
            )
            .await?;
            PendingTransport::StreamableHttpWithOAuth {
                transport,
                oauth_persistor,
            }
        } else {
            let mut http_config = StreamableHttpClientTransportConfig::with_uri(url.to_string());
            if let Some(bearer_token) = bearer_token.clone() {
                http_config = http_config.auth_header(bearer_token);
            }

            let http_client =
                apply_default_headers(reqwest::Client::builder(), &default_headers).build()?;

            let transport = StreamableHttpClientTransport::with_client(http_client, http_config);
            PendingTransport::StreamableHttp { transport }
        };
        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
        })
    }

    /// Perform the initialization handshake with the MCP server.
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle#initialization
    pub async fn initialize(
        &self,
        params: InitializeRequestParams,
        timeout: Option<Duration>,
    ) -> Result<InitializeResult> {
        let rmcp_params: InitializeRequestParam = convert_to_rmcp(params.clone())?;
        let client_handler = LoggingClientHandler::new(rmcp_params);

        let (transport, oauth_persistor) = {
            let mut guard = self.state.lock().await;
            match &mut *guard {
                ClientState::Connecting { transport } => match transport.take() {
                    Some(PendingTransport::ChildProcess(transport)) => (
                        service::serve_client(client_handler.clone(), transport).boxed(),
                        None,
                    ),
                    Some(PendingTransport::StreamableHttp { transport }) => (
                        service::serve_client(client_handler.clone(), transport).boxed(),
                        None,
                    ),
                    Some(PendingTransport::StreamableHttpWithOAuth {
                        transport,
                        oauth_persistor,
                    }) => (
                        service::serve_client(client_handler.clone(), transport).boxed(),
                        Some(oauth_persistor),
                    ),
                    None => return Err(anyhow!("client already initializing")),
                },
                ClientState::Ready { .. } => return Err(anyhow!("client already initialized")),
            }
        };

        let service = match timeout {
            Some(duration) => time::timeout(duration, transport)
                .await
                .map_err(|_| anyhow!("timed out handshaking with MCP server after {duration:?}"))?
                .map_err(|err| anyhow!("handshaking with MCP server failed: {err}"))?,
            None => transport
                .await
                .map_err(|err| anyhow!("handshaking with MCP server failed: {err}"))?,
        };

        let initialize_result_rmcp = service
            .peer()
            .peer_info()
            .ok_or_else(|| anyhow!("handshake succeeded but server info was missing"))?;
        let initialize_result = convert_to_mcp(initialize_result_rmcp)?;

        {
            let mut guard = self.state.lock().await;
            *guard = ClientState::Ready {
                service: Arc::new(service),
                oauth: oauth_persistor.clone(),
            };
        }

        if let Some(runtime) = oauth_persistor
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens after initialize: {error}");
        }

        Ok(initialize_result)
    }

    pub async fn list_tools(
        &self,
        params: Option<ListToolsRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsResult> {
        let service = self.service().await?;
        let rmcp_params = params
            .map(convert_to_rmcp::<_, PaginatedRequestParam>)
            .transpose()?;

        let fut = service.list_tools(rmcp_params);
        let result = run_with_timeout(fut, timeout, "tools/list").await?;
        let converted = convert_to_mcp(result)?;
        self.persist_oauth_tokens().await;
        Ok(converted)
    }

    pub async fn list_resources(
        &self,
        params: Option<ListResourcesRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListResourcesResult> {
        let service = self.service().await?;
        let rmcp_params = params
            .map(convert_to_rmcp::<_, PaginatedRequestParam>)
            .transpose()?;

        let fut = service.list_resources(rmcp_params);
        let result = run_with_timeout(fut, timeout, "resources/list").await?;
        let converted = convert_to_mcp(result)?;
        self.persist_oauth_tokens().await;
        Ok(converted)
    }

    pub async fn list_resource_templates(
        &self,
        params: Option<ListResourceTemplatesRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListResourceTemplatesResult> {
        let service = self.service().await?;
        let rmcp_params = params
            .map(convert_to_rmcp::<_, PaginatedRequestParam>)
            .transpose()?;

        let fut = service.list_resource_templates(rmcp_params);
        let result = run_with_timeout(fut, timeout, "resources/templates/list").await?;
        let converted = convert_to_mcp(result)?;
        self.persist_oauth_tokens().await;
        Ok(converted)
    }

    pub async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        timeout: Option<Duration>,
    ) -> Result<ReadResourceResult> {
        let service = self.service().await?;
        let rmcp_params: ReadResourceRequestParam = convert_to_rmcp(params)?;
        let fut = service.read_resource(rmcp_params);
        let result = run_with_timeout(fut, timeout, "resources/read").await?;
        let converted = convert_to_mcp(result)?;
        self.persist_oauth_tokens().await;
        Ok(converted)
    }

    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        let service = self.service().await?;
        let params = CallToolRequestParams { arguments, name };
        let rmcp_params: CallToolRequestParam = convert_to_rmcp(params)?;
        let fut = service.call_tool(rmcp_params);
        let rmcp_result = run_with_timeout(fut, timeout, "tools/call").await?;
        let converted = convert_call_tool_result(rmcp_result)?;
        self.persist_oauth_tokens().await;
        Ok(converted)
    }

    async fn service(&self) -> Result<Arc<RunningService<RoleClient, LoggingClientHandler>>> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready { service, .. } => Ok(Arc::clone(service)),
            ClientState::Connecting { .. } => Err(anyhow!("MCP client not initialized")),
        }
    }

    async fn oauth_persistor(&self) -> Option<OAuthPersistor> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready {
                oauth: Some(runtime),
                service: _,
            } => Some(runtime.clone()),
            _ => None,
        }
    }

    /// This should be called after every tool call so that if a given tool call triggered
    /// a refresh of the OAuth tokens, they are persisted.
    async fn persist_oauth_tokens(&self) {
        if let Some(runtime) = self.oauth_persistor().await
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens: {error}");
        }
    }
}

async fn create_oauth_transport_and_runtime(
    server_name: &str,
    url: &str,
    initial_tokens: StoredOAuthTokens,
    credentials_store: OAuthCredentialsStoreMode,
    default_headers: HeaderMap,
) -> Result<(
    StreamableHttpClientTransport<AuthClient<reqwest::Client>>,
    OAuthPersistor,
)> {
    let http_client =
        apply_default_headers(reqwest::Client::builder(), &default_headers).build()?;
    let mut oauth_state = OAuthState::new(url.to_string(), Some(http_client.clone())).await?;

    oauth_state
        .set_credentials(
            &initial_tokens.client_id,
            initial_tokens.token_response.0.clone(),
        )
        .await?;

    let manager = match oauth_state {
        OAuthState::Authorized(manager) => manager,
        OAuthState::Unauthorized(manager) => manager,
        OAuthState::Session(_) | OAuthState::AuthorizedHttpClient(_) => {
            return Err(anyhow!("unexpected OAuth state during client setup"));
        }
    };

    let auth_client = AuthClient::new(http_client, manager);
    let auth_manager = auth_client.auth_manager.clone();

    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(url.to_string()),
    );

    let runtime = OAuthPersistor::new(
        server_name.to_string(),
        url.to_string(),
        auth_manager,
        credentials_store,
        Some(initial_tokens),
    );

    Ok((transport, runtime))
}

/// Platform-specific program resolution for MCP server execution.
///
/// Resolves executable paths differently based on the operating system:
/// - **Windows**: Uses `which` crate to find executables including scripts (.cmd, .bat)
/// - **Unix**: Returns the program unchanged (native PATH resolution handles it)
mod program_resolver {
    use super::*;
    #[cfg(windows)]
    use std::env;
    #[cfg(windows)]
    use tracing::debug;

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
        // Get current directory for relative path resolution
        let cwd = env::current_dir()
            .map_err(|e| std::io::Error::other(format!("Failed to get current directory: {e}")))?;

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
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

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

            let mcp_env = create_env_for_mcp_server(Some(extra_env), &[]);

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
