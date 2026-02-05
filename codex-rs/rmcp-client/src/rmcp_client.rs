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
use futures::future::BoxFuture;
use reqwest::header::HeaderMap;
use rmcp::model::CallToolRequestParam;
use rmcp::model::CallToolResult;
use rmcp::model::ClientNotification;
use rmcp::model::ClientRequest;
use rmcp::model::CreateElicitationRequestParam;
use rmcp::model::CreateElicitationResult;
use rmcp::model::CustomNotification;
use rmcp::model::CustomRequest;
use rmcp::model::Extensions;
use rmcp::model::InitializeRequestParam;
use rmcp::model::InitializeResult;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParam;
use rmcp::model::ReadResourceRequestParam;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use rmcp::model::ServerResult;
use rmcp::model::Tool;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::service::{self};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use serde_json::Value;
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
use crate::program_resolver;
use crate::utils::apply_default_headers;
use crate::utils::build_default_headers;
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

pub type Elicitation = CreateElicitationRequestParam;
pub type ElicitationResponse = CreateElicitationResult;

/// Interface for sending elicitation requests to the UI and awaiting a response.
pub type SendElicitation = Box<
    dyn Fn(RequestId, Elicitation) -> BoxFuture<'static, Result<ElicitationResponse>> + Send + Sync,
>;

pub struct ToolWithConnectorId {
    pub tool: Tool,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
}

pub struct ListToolsWithConnectorIdResult {
    pub next_cursor: Option<String>,
    pub tools: Vec<ToolWithConnectorId>,
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
        params: InitializeRequestParam,
        timeout: Option<Duration>,
        send_elicitation: SendElicitation,
    ) -> Result<InitializeResult> {
        let client_handler = LoggingClientHandler::new(params.clone(), send_elicitation);

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
        let initialize_result = initialize_result_rmcp.clone();

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
        params: Option<PaginatedRequestParam>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsResult> {
        self.refresh_oauth_if_needed().await;
        let service = self.service().await?;
        let fut = service.list_tools(params);
        let result = run_with_timeout(fut, timeout, "tools/list").await?;
        self.persist_oauth_tokens().await;
        Ok(result)
    }

    pub async fn list_tools_with_connector_ids(
        &self,
        params: Option<PaginatedRequestParam>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsWithConnectorIdResult> {
        self.refresh_oauth_if_needed().await;
        let service = self.service().await?;

        let fut = service.list_tools(params);
        let result = run_with_timeout(fut, timeout, "tools/list").await?;
        let tools = result
            .tools
            .into_iter()
            .map(|tool| {
                let meta = tool.meta.as_ref();
                let connector_id = Self::meta_string(meta, "connector_id");
                let connector_name = Self::meta_string(meta, "connector_name")
                    .or_else(|| Self::meta_string(meta, "connector_display_name"));
                Ok(ToolWithConnectorId {
                    tool,
                    connector_id,
                    connector_name,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.persist_oauth_tokens().await;
        Ok(ListToolsWithConnectorIdResult {
            next_cursor: result.next_cursor,
            tools,
        })
    }

    fn meta_string(meta: Option<&rmcp::model::Meta>, key: &str) -> Option<String> {
        meta.and_then(|meta| meta.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    pub async fn list_resources(
        &self,
        params: Option<PaginatedRequestParam>,
        timeout: Option<Duration>,
    ) -> Result<ListResourcesResult> {
        self.refresh_oauth_if_needed().await;
        let service = self.service().await?;

        let fut = service.list_resources(params);
        let result = run_with_timeout(fut, timeout, "resources/list").await?;
        self.persist_oauth_tokens().await;
        Ok(result)
    }

    pub async fn list_resource_templates(
        &self,
        params: Option<PaginatedRequestParam>,
        timeout: Option<Duration>,
    ) -> Result<ListResourceTemplatesResult> {
        self.refresh_oauth_if_needed().await;
        let service = self.service().await?;

        let fut = service.list_resource_templates(params);
        let result = run_with_timeout(fut, timeout, "resources/templates/list").await?;
        self.persist_oauth_tokens().await;
        Ok(result)
    }

    pub async fn read_resource(
        &self,
        params: ReadResourceRequestParam,
        timeout: Option<Duration>,
    ) -> Result<ReadResourceResult> {
        self.refresh_oauth_if_needed().await;
        let service = self.service().await?;
        let fut = service.read_resource(params);
        let result = run_with_timeout(fut, timeout, "resources/read").await?;
        self.persist_oauth_tokens().await;
        Ok(result)
    }

    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        self.refresh_oauth_if_needed().await;
        let service = self.service().await?;
        let arguments = match arguments {
            Some(Value::Object(map)) => Some(map),
            Some(other) => {
                return Err(anyhow!(
                    "MCP tool arguments must be a JSON object, got {other}"
                ));
            }
            None => None,
        };
        let rmcp_params = CallToolRequestParam {
            name: name.into(),
            arguments,
        };
        let fut = service.call_tool(rmcp_params);
        let result = run_with_timeout(fut, timeout, "tools/call").await?;
        self.persist_oauth_tokens().await;
        Ok(result)
    }

    pub async fn send_custom_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<()> {
        let service: Arc<RunningService<RoleClient, LoggingClientHandler>> = self.service().await?;
        service
            .send_notification(ClientNotification::CustomNotification(CustomNotification {
                method: method.to_string(),
                params,
                extensions: Extensions::new(),
            }))
            .await?;
        Ok(())
    }

    pub async fn send_custom_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<ServerResult> {
        let service: Arc<RunningService<RoleClient, LoggingClientHandler>> = self.service().await?;
        let response = service
            .send_request(ClientRequest::CustomRequest(CustomRequest::new(
                method, params,
            )))
            .await?;
        Ok(response)
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

    async fn refresh_oauth_if_needed(&self) {
        if let Some(runtime) = self.oauth_persistor().await
            && let Err(error) = runtime.refresh_if_needed().await
        {
            warn!("failed to refresh OAuth tokens: {error}");
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
