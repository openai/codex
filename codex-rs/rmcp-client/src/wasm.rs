use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use futures::future::BoxFuture;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

pub use codex_protocol::protocol::McpAuthStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamableHttpOAuthDiscovery {
    pub scopes_supported: Option<Vec<String>>,
}

pub async fn determine_streamable_http_auth_status(
    _server_name: &str,
    _url: &str,
    bearer_token_env_var: Option<&str>,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    _store_mode: OAuthCredentialsStoreMode,
) -> Result<McpAuthStatus> {
    if bearer_token_env_var.is_some() {
        return Ok(McpAuthStatus::BearerToken);
    }

    let has_auth_header = http_headers
        .into_iter()
        .flatten()
        .chain(env_http_headers.into_iter().flatten())
        .any(|(key, _)| key.eq_ignore_ascii_case("authorization"));
    if has_auth_header {
        return Ok(McpAuthStatus::BearerToken);
    }

    Ok(McpAuthStatus::Unsupported)
}

pub async fn supports_oauth_login(_url: &str) -> Result<bool> {
    Ok(false)
}

pub async fn discover_streamable_http_oauth(
    _url: &str,
    _http_headers: Option<HashMap<String, String>>,
    _env_http_headers: Option<HashMap<String, String>>,
) -> Result<Option<StreamableHttpOAuthDiscovery>> {
    Ok(None)
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OAuthCredentialsStoreMode {
    #[default]
    Auto,
    File,
    Keyring,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredOAuthTokens {
    pub server_name: String,
    pub url: String,
    pub client_id: String,
    pub token_response: WrappedOAuthTokenResponse,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WrappedOAuthTokenResponse(pub serde_json::Value);

pub fn delete_oauth_tokens(
    _server_name: &str,
    _url: &str,
    _store_mode: OAuthCredentialsStoreMode,
) -> Result<bool> {
    Ok(false)
}

pub fn save_oauth_tokens(
    _server_name: &str,
    _tokens: &StoredOAuthTokens,
    _store_mode: OAuthCredentialsStoreMode,
) -> Result<()> {
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderError {
    error: Option<String>,
    error_description: Option<String>,
}

impl OAuthProviderError {
    pub fn new(error: Option<String>, error_description: Option<String>) -> Self {
        Self {
            error,
            error_description,
        }
    }
}

impl std::fmt::Display for OAuthProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.error.as_deref(), self.error_description.as_deref()) {
            (Some(error), Some(error_description)) => {
                write!(f, "OAuth provider returned `{error}`: {error_description}")
            }
            (Some(error), None) => write!(f, "OAuth provider returned `{error}`"),
            (None, Some(error_description)) => write!(f, "OAuth error: {error_description}"),
            (None, None) => write!(f, "OAuth provider returned an error"),
        }
    }
}

impl std::error::Error for OAuthProviderError {}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login(
    _server_name: &str,
    _server_url: &str,
    _store_mode: OAuthCredentialsStoreMode,
    _http_headers: Option<HashMap<String, String>>,
    _env_http_headers: Option<HashMap<String, String>>,
    _scopes: &[String],
    _oauth_resource: Option<&str>,
    _callback_port: Option<u16>,
    _callback_url: Option<&str>,
) -> Result<()> {
    Err(anyhow!("MCP OAuth login is unavailable on wasm32"))
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_silent(
    _server_name: &str,
    _server_url: &str,
    _store_mode: OAuthCredentialsStoreMode,
    _http_headers: Option<HashMap<String, String>>,
    _env_http_headers: Option<HashMap<String, String>>,
    _scopes: &[String],
    _oauth_resource: Option<&str>,
    _callback_port: Option<u16>,
    _callback_url: Option<&str>,
) -> Result<()> {
    Err(anyhow!("MCP OAuth login is unavailable on wasm32"))
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_return_url(
    _server_name: &str,
    _server_url: &str,
    _store_mode: OAuthCredentialsStoreMode,
    _http_headers: Option<HashMap<String, String>>,
    _env_http_headers: Option<HashMap<String, String>>,
    _scopes: &[String],
    _oauth_resource: Option<&str>,
    _callback_port: Option<u16>,
    _callback_url: Option<&str>,
) -> Result<OauthLoginHandle> {
    Err(anyhow!("MCP OAuth login is unavailable on wasm32"))
}

pub struct OauthLoginHandle {
    authorization_url: String,
}

impl OauthLoginHandle {
    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    pub fn into_parts(self) -> (String, futures::channel::oneshot::Receiver<Result<()>>) {
        let (_tx, rx) = futures::channel::oneshot::channel();
        (self.authorization_url, rx)
    }

    pub async fn wait(self) -> Result<()> {
        Err(anyhow!("MCP OAuth login is unavailable on wasm32"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RequestId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Elicitation {
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationResponse {
    pub action: ElicitationAction,
    pub content: Option<serde_json::Value>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

pub type SendElicitation = Box<
    dyn Fn(RequestId, Elicitation) -> BoxFuture<'static, Result<ElicitationResponse>> + Send + Sync,
>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ToolDefinition {
    #[serde(default)]
    pub name: String,
}

pub struct ToolWithConnectorId {
    pub tool: ToolDefinition,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    pub connector_description: Option<String>,
}

pub struct ListToolsWithConnectorIdResult {
    pub next_cursor: Option<String>,
    pub tools: Vec<ToolWithConnectorId>,
}

pub struct RmcpClient;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InitializeRequestParams;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InitializeResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaginatedRequestParams {
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListResourcesResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListResourceTemplatesResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReadResourceRequestParams;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReadResourceResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CallToolResult;

impl RmcpClient {
    pub async fn new_stdio_client(
        _program: OsString,
        _args: Vec<OsString>,
        _env: Option<HashMap<OsString, OsString>>,
        _env_vars: &[String],
        _cwd: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        Err(std::io::Error::other(
            "MCP stdio transport is unavailable on wasm32",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new_streamable_http_client(
        _server_name: &str,
        _url: &str,
        _bearer_token: Option<String>,
        _http_headers: Option<HashMap<String, String>>,
        _env_http_headers: Option<HashMap<String, String>>,
        _store_mode: OAuthCredentialsStoreMode,
    ) -> Result<Self> {
        Err(anyhow!("MCP HTTP transport is unavailable on wasm32"))
    }

    pub async fn initialize(
        &self,
        _params: InitializeRequestParams,
        _timeout: Option<Duration>,
        _send_elicitation: SendElicitation,
    ) -> Result<InitializeResult> {
        Err(anyhow!("MCP initialize is unavailable on wasm32"))
    }

    pub async fn list_tools_with_connector_ids(
        &self,
        _params: Option<PaginatedRequestParams>,
        _timeout: Option<Duration>,
    ) -> Result<ListToolsWithConnectorIdResult> {
        Err(anyhow!("MCP tools are unavailable on wasm32"))
    }

    pub async fn list_resources(
        &self,
        _params: Option<PaginatedRequestParams>,
        _timeout: Option<Duration>,
    ) -> Result<ListResourcesResult> {
        Err(anyhow!("MCP resources are unavailable on wasm32"))
    }

    pub async fn list_resource_templates(
        &self,
        _params: Option<PaginatedRequestParams>,
        _timeout: Option<Duration>,
    ) -> Result<ListResourceTemplatesResult> {
        Err(anyhow!("MCP resource templates are unavailable on wasm32"))
    }

    pub async fn read_resource(
        &self,
        _params: ReadResourceRequestParams,
        _timeout: Option<Duration>,
    ) -> Result<ReadResourceResult> {
        Err(anyhow!("MCP resources are unavailable on wasm32"))
    }

    pub async fn call_tool(
        &self,
        _name: String,
        _arguments: Option<serde_json::Value>,
        _meta: Option<serde_json::Value>,
        _timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        Err(anyhow!("MCP tools are unavailable on wasm32"))
    }
}
