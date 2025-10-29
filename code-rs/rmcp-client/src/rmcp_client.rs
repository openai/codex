use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
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
use mcp_types::ListToolsRequestParams;
use mcp_types::ListToolsResult;
use mcp_types::MCP_SCHEMA_VERSION;
use rmcp::model::CallToolRequestParam;
use rmcp::model::InitializeRequestParam;
use rmcp::model::PaginatedRequestParam;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::service::{self};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time;
use tracing::info;
use tracing::warn;

use crate::logging_client_handler::LoggingClientHandler;
use crate::utils::convert_call_tool_result;
use crate::utils::convert_to_mcp;
use crate::utils::convert_to_rmcp;
use crate::utils::create_env_for_mcp_server;
use crate::utils::run_with_timeout;

enum PendingTransport {
    ChildProcess(TokioChildProcess),
    StreamableHttp(StreamableHttpClientTransport<reqwest::Client>),
}

enum ClientState {
    Connecting {
        transport: Option<PendingTransport>,
    },
    Ready {
        service: Arc<RunningService<RoleClient, LoggingClientHandler>>,
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
    ) -> io::Result<Self> {
        let program_name = program.to_string_lossy().into_owned();
        let mut command = Command::new(&program);
        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env_clear()
            .envs(create_env_for_mcp_server(env))
            .args(&args);

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

    pub fn new_streamable_http_client(url: String, bearer_token: Option<String>) -> Result<Self> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(url);
        if let Some(token) = bearer_token {
            config = config.auth_header(format!("Bearer {token}"));
        }

        let transport = StreamableHttpClientTransport::from_config(config);

        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(PendingTransport::StreamableHttp(transport)),
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
        let transport = {
            let mut guard = self.state.lock().await;
            match &mut *guard {
                ClientState::Connecting { transport } => transport
                    .take()
                    .ok_or_else(|| anyhow!("client already initializing"))?,
                ClientState::Ready { .. } => {
                    return Err(anyhow!("client already initialized"));
                }
            }
        };

        let client_info = convert_to_rmcp::<_, InitializeRequestParam>(params.clone())?;
        let client_handler = LoggingClientHandler::new(client_info);
        let service_future = match transport {
            PendingTransport::ChildProcess(transport) => {
                service::serve_client(client_handler.clone(), transport).boxed()
            }
            PendingTransport::StreamableHttp(transport) => {
                service::serve_client(client_handler, transport).boxed()
            }
        };

        let service = match timeout {
            Some(duration) => match time::timeout(duration, service_future).await {
                Ok(Ok(service)) => service,
                Ok(Err(err)) => return Err(handshake_failed_error(err)),
                Err(_) => return Err(handshake_timeout_error(duration)),
            },
            None => match service_future.await {
                Ok(service) => service,
                Err(err) => return Err(handshake_failed_error(err)),
            },
        };

        let initialize_result_rmcp = service
            .peer()
            .peer_info()
            .ok_or_else(|| anyhow!("handshake succeeded but server info was missing"))?;
        let initialize_result: InitializeResult = convert_to_mcp(initialize_result_rmcp)?;

        if initialize_result.protocol_version != MCP_SCHEMA_VERSION {
            let reported_version = initialize_result.protocol_version.clone();
            return Err(anyhow!(
                "MCP server reported protocol version {reported_version}, but this client expects {}. Update either side so both speak the same schema.",
                MCP_SCHEMA_VERSION
            ));
        }

        {
            let mut guard = self.state.lock().await;
            *guard = ClientState::Ready {
                service: Arc::new(service),
            };
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
        convert_to_mcp(result)
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
        convert_call_tool_result(rmcp_result)
    }

    async fn service(&self) -> Result<Arc<RunningService<RoleClient, LoggingClientHandler>>> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready { service } => Ok(Arc::clone(service)),
            ClientState::Connecting { .. } => Err(anyhow!("MCP client not initialized")),
        }
    }

    pub async fn shutdown(&self) {
        if let Ok(service) = self.service().await {
            service.cancellation_token().cancel();
        }
    }
}

fn handshake_failed_error(err: impl Into<anyhow::Error>) -> anyhow::Error {
    let err = err.into();
    anyhow!(
        "handshaking with MCP server failed: {err} (this client supports MCP schema version {MCP_SCHEMA_VERSION})"
    )
}

fn handshake_timeout_error(duration: Duration) -> anyhow::Error {
    anyhow!(
        "timed out handshaking with MCP server after {duration:?} (expected MCP schema version {MCP_SCHEMA_VERSION})"
    )
}
