use std::collections::HashMap;
use std::env;
use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_api::SharedAuthProvider;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_rmcp_client::RmcpClient;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;

use super::CODEX_APPS_LOAD_TIMEOUT;
use super::CodexAppsCacheContext;
use super::MAX_CODEX_APPS_UPSTREAM_POST_RESPONSE_BYTES;
use super::elicitation_bridge::AppsElicitationBridge;

const CODEX_CONNECTORS_TOKEN_ENV_VAR: &str = "CODEX_CONNECTORS_TOKEN";
const PRODUCT_SKU_HEADER: &str = "X-OpenAI-Product-Sku";
const UPSTREAM_SERVER_NAME: &str = "codex_apps_upstream";

/// Ordinary MCP resource namespace for orchestrator-owned skills.
pub use codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME as CODEX_APPS_RESOURCE_MCP_SERVER_NAME;

/// Inputs that select and authenticate upstream Apps MCP connections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexAppsConnectConfig {
    pub chatgpt_base_url: String,
    pub product_sku: Option<String>,
    pub oauth_credentials_store_mode: OAuthCredentialsStoreMode,
    pub auth_keyring_backend_kind: AuthKeyringBackendKind,
    pub(crate) auth_elicitation_enabled: bool,
    pub(crate) cache_context: Option<CodexAppsCacheContext>,
}

impl CodexAppsConnectConfig {
    pub fn new(
        chatgpt_base_url: String,
        product_sku: Option<String>,
        oauth_credentials_store_mode: OAuthCredentialsStoreMode,
        auth_keyring_backend_kind: AuthKeyringBackendKind,
    ) -> Self {
        Self {
            chatgpt_base_url,
            product_sku,
            oauth_credentials_store_mode,
            auth_keyring_backend_kind,
            auth_elicitation_enabled: false,
            cache_context: None,
        }
    }

    /// Controls whether hosted MCP connections advertise standard MCP elicitation.
    pub fn with_auth_elicitation(mut self, enabled: bool) -> Self {
        self.auth_elicitation_enabled = enabled;
        self
    }

    pub fn with_cache_context(mut self, cache_context: CodexAppsCacheContext) -> Self {
        self.cache_context = Some(cache_context);
        self
    }

    pub(crate) fn scoped_cache_context(&self) -> Option<super::cache::ScopedCodexAppsCacheContext> {
        self.cache_context.clone().map(|cache_context| {
            cache_context.scoped(self.upstream_url(), self.product_sku.clone())
        })
    }

    pub(crate) fn upstream_url(&self) -> String {
        hosted_plugin_runtime_url(&self.chatgpt_base_url)
    }
}

pub(crate) async fn connect_upstream(
    config: &CodexAppsConnectConfig,
    bearer_token: Option<String>,
    auth_provider: SharedAuthProvider,
    elicitation_bridge: Arc<AppsElicitationBridge>,
) -> Result<Arc<RmcpClient>> {
    let http_headers = config
        .product_sku
        .as_ref()
        .map(|product_sku| HashMap::from([(PRODUCT_SKU_HEADER.to_string(), product_sku.clone())]));
    let upstream_url = config.upstream_url();
    let max_post_response_body_bytes =
        NonZeroUsize::new(MAX_CODEX_APPS_UPSTREAM_POST_RESPONSE_BYTES)
            .context("Codex Apps upstream POST response limit must be non-zero")?;
    let client = Arc::new(
        RmcpClient::new_streamable_http_client_with_post_response_body_limit(
            UPSTREAM_SERVER_NAME,
            &upstream_url,
            bearer_token,
            http_headers,
            /*env_http_headers*/ None,
            config.oauth_credentials_store_mode,
            config.auth_keyring_backend_kind,
            Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
            Some(auth_provider),
            max_post_response_body_bytes,
        )
        .await
        .with_context(|| format!("failed to connect to Codex Apps MCP at `{upstream_url}`"))?,
    );

    let initialize_params = InitializeRequestParams::new(
        AppsElicitationBridge::upstream_capabilities(config.auth_elicitation_enabled),
        Implementation::new("codex-apps", env!("CARGO_PKG_VERSION")).with_title("Codex Apps"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18);
    let send_elicitation = Box::new(move |request_id, elicitation| {
        let elicitation_bridge = Arc::clone(&elicitation_bridge);
        Box::pin(async move { elicitation_bridge.forward(request_id, elicitation).await }) as _
    });
    if let Err(error) = client
        .initialize(
            initialize_params,
            Some(CODEX_APPS_LOAD_TIMEOUT),
            send_elicitation,
        )
        .await
    {
        client.shutdown().await;
        return Err(error).context("failed to initialize Codex Apps MCP");
    }

    Ok(client)
}

pub(super) fn connectors_bearer_token() -> Result<Option<String>> {
    resolve_connectors_bearer_token(env::var(CODEX_CONNECTORS_TOKEN_ENV_VAR))
}

fn resolve_connectors_bearer_token(
    value: std::result::Result<String, env::VarError>,
) -> Result<Option<String>> {
    match value {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            bail!("environment variable {CODEX_CONNECTORS_TOKEN_ENV_VAR} is not valid Unicode")
        }
    }
}

fn hosted_plugin_runtime_url(base_url: &str) -> String {
    let mut base_url = base_url.trim_end_matches('/').to_string();
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    let base_url = if base_url.contains("/backend-api") || base_url.contains("/api/codex") {
        base_url
    } else {
        format!("{base_url}/api/codex")
    };
    format!("{base_url}/ps/mcp")
}

#[cfg(test)]
mod tests {
    use super::resolve_connectors_bearer_token;

    #[test]
    fn blank_debug_token_is_absent() {
        assert_eq!(
            resolve_connectors_bearer_token(Ok("  ".to_string())).expect("resolve token"),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn invalid_unicode_debug_token_is_rejected() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let error = resolve_connectors_bearer_token(Err(std::env::VarError::NotUnicode(
            OsString::from_vec(vec![0xff]),
        )))
        .expect_err("invalid Unicode token must fail before cache lookup");

        assert!(error.to_string().contains("is not valid Unicode"));
    }
}
