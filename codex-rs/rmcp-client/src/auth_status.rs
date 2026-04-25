use std::collections::HashMap;
use std::time::Duration;

use anyhow::Error;
use anyhow::Result;
use codex_protocol::protocol::McpAuthStatus;
use reqwest::Client;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderMap;
use reqwest::header::WWW_AUTHENTICATE;
use serde::Deserialize;
use tracing::debug;

use crate::oauth::has_oauth_tokens;
use crate::utils::apply_default_headers;
use crate::utils::build_default_headers;
use codex_config::types::OAuthCredentialsStoreMode;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const OAUTH_DISCOVERY_HEADER: &str = "MCP-Protocol-Version";
const OAUTH_DISCOVERY_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamableHttpOAuthDiscovery {
    pub scopes_supported: Option<Vec<String>>,
}

/// Determine the authentication status for a streamable HTTP MCP server.
pub async fn determine_streamable_http_auth_status(
    server_name: &str,
    url: &str,
    bearer_token_env_var: Option<&str>,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<McpAuthStatus> {
    if bearer_token_env_var.is_some() {
        return Ok(McpAuthStatus::BearerToken);
    }

    let default_headers = build_default_headers(http_headers, env_http_headers)?;
    if default_headers.contains_key(AUTHORIZATION) {
        return Ok(McpAuthStatus::BearerToken);
    }

    if has_oauth_tokens(server_name, url, store_mode)? {
        return Ok(McpAuthStatus::OAuth);
    }

    match discover_streamable_http_oauth_with_headers(url, &default_headers).await {
        Ok(Some(_)) => Ok(McpAuthStatus::NotLoggedIn),
        Ok(None) => Ok(McpAuthStatus::Unsupported),
        Err(error) => {
            debug!(
                "failed to detect OAuth support for MCP server `{server_name}` at {url}: {error:?}"
            );
            Ok(McpAuthStatus::Unsupported)
        }
    }
}

/// Attempt to determine whether a streamable HTTP MCP server advertises OAuth login.
pub async fn supports_oauth_login(url: &str) -> Result<bool> {
    Ok(discover_streamable_http_oauth(
        url, /*http_headers*/ None, /*env_http_headers*/ None,
    )
    .await?
    .is_some())
}

pub async fn discover_streamable_http_oauth(
    url: &str,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
) -> Result<Option<StreamableHttpOAuthDiscovery>> {
    let default_headers = build_default_headers(http_headers, env_http_headers)?;
    discover_streamable_http_oauth_with_headers(url, &default_headers).await
}

async fn discover_streamable_http_oauth_with_headers(
    url: &str,
    default_headers: &HeaderMap,
) -> Result<Option<StreamableHttpOAuthDiscovery>> {
    let base_url = Url::parse(url)?;

    // Use no_proxy to avoid a bug in the system-configuration crate that
    // can result in a panic. See #8912.
    let builder = Client::builder().timeout(DISCOVERY_TIMEOUT).no_proxy();
    let client = apply_default_headers(builder, default_headers).build()?;

    if let Some(metadata) = discover_authorization_metadata(&client, &base_url).await? {
        return Ok(Some(metadata.into()));
    }

    if let Some(resource_metadata) = discover_resource_metadata(&client, &base_url).await? {
        for authorization_server in resource_metadata.authorization_servers() {
            let authorization_server = authorization_server.trim();
            if authorization_server.is_empty() {
                continue;
            }

            let authorization_server_url = match Url::parse(authorization_server) {
                Ok(url) => url,
                Err(_) => match base_url.join(authorization_server) {
                    Ok(url) => url,
                    Err(err) => {
                        debug!(
                            "failed to resolve authorization server URL `{authorization_server}`: {err}"
                        );
                        continue;
                    }
                },
            };

            let metadata = if authorization_server_url.path().contains("/.well-known/") {
                fetch_authorization_metadata(&client, &authorization_server_url).await?
            } else {
                discover_authorization_metadata(&client, &authorization_server_url).await?
            };
            if let Some(metadata) = metadata {
                return Ok(Some(metadata.into()));
            }
        }
    }

    Ok(None)
}

async fn discover_authorization_metadata(
    client: &Client,
    base_url: &Url,
) -> Result<Option<OAuthDiscoveryMetadata>> {
    let mut last_error: Option<Error> = None;

    for discovery_url in authorization_metadata_urls(base_url) {
        match fetch_authorization_metadata(client, &discovery_url).await {
            Ok(Some(metadata)) => return Ok(Some(metadata)),
            Ok(None) => {}
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    if let Some(err) = last_error {
        debug!("OAuth discovery requests failed for {base_url}: {err:?}");
    }

    Ok(None)
}

async fn fetch_authorization_metadata(
    client: &Client,
    discovery_url: &Url,
) -> Result<Option<OAuthDiscoveryMetadata>> {
    let response = client
        .get(discovery_url.clone())
        .header(OAUTH_DISCOVERY_HEADER, OAUTH_DISCOVERY_VERSION)
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        return Ok(None);
    }

    let metadata = response.json::<OAuthDiscoveryMetadata>().await?;
    if metadata.authorization_endpoint.is_some() && metadata.token_endpoint.is_some() {
        return Ok(Some(metadata));
    }

    Ok(None)
}

async fn discover_resource_metadata(
    client: &Client,
    base_url: &Url,
) -> Result<Option<ResourceServerMetadata>> {
    let mut last_error: Option<Error> = None;

    let mut candidates = vec![base_url.clone()];
    candidates.extend(protected_resource_metadata_urls(base_url));

    for candidate in candidates {
        match fetch_resource_metadata_candidate(client, base_url, &candidate).await {
            Ok(Some(metadata)) => return Ok(Some(metadata)),
            Ok(None) => {}
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    if let Some(err) = last_error {
        debug!("OAuth protected resource discovery failed for {base_url}: {err:?}");
    }

    Ok(None)
}

async fn fetch_resource_metadata_candidate(
    client: &Client,
    base_url: &Url,
    url: &Url,
) -> Result<Option<ResourceServerMetadata>> {
    let response = client
        .get(url.clone())
        .header(OAUTH_DISCOVERY_HEADER, OAUTH_DISCOVERY_VERSION)
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => {
            let metadata = response.json::<ResourceServerMetadata>().await?;
            Ok(metadata.has_authorization_servers().then_some(metadata))
        }
        StatusCode::UNAUTHORIZED => {
            let mut resource_metadata_url = None;
            for value in response.headers().get_all(WWW_AUTHENTICATE) {
                let Ok(value) = value.to_str() else {
                    continue;
                };
                if let Some(url) = extract_resource_metadata_url_from_header(value, base_url) {
                    resource_metadata_url = Some(url);
                    break;
                }
            }

            let Some(resource_metadata_url) = resource_metadata_url else {
                return Ok(None);
            };
            fetch_resource_metadata_from_url(client, &resource_metadata_url).await
        }
        _ => Ok(None),
    }
}

async fn fetch_resource_metadata_from_url(
    client: &Client,
    url: &Url,
) -> Result<Option<ResourceServerMetadata>> {
    let response = client
        .get(url.clone())
        .header(OAUTH_DISCOVERY_HEADER, OAUTH_DISCOVERY_VERSION)
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        return Ok(None);
    }

    let metadata = response.json::<ResourceServerMetadata>().await?;
    Ok(metadata.has_authorization_servers().then_some(metadata))
}

#[derive(Debug, Deserialize)]
struct OAuthDiscoveryMetadata {
    #[serde(default)]
    authorization_endpoint: Option<String>,
    #[serde(default)]
    token_endpoint: Option<String>,
    #[serde(default)]
    scopes_supported: Option<Vec<String>>,
}

impl From<OAuthDiscoveryMetadata> for StreamableHttpOAuthDiscovery {
    fn from(metadata: OAuthDiscoveryMetadata) -> Self {
        Self {
            scopes_supported: normalize_scopes(metadata.scopes_supported),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ResourceServerMetadata {
    #[serde(default)]
    authorization_server: Option<String>,
    #[serde(default)]
    authorization_servers: Option<Vec<String>>,
}

impl ResourceServerMetadata {
    fn has_authorization_servers(&self) -> bool {
        self.authorization_server
            .as_deref()
            .is_some_and(|server| !server.trim().is_empty())
            || self
                .authorization_servers
                .as_ref()
                .is_some_and(|servers| servers.iter().any(|server| !server.trim().is_empty()))
    }

    fn authorization_servers(&self) -> impl Iterator<Item = &str> {
        self.authorization_server.as_deref().into_iter().chain(
            self.authorization_servers
                .as_deref()
                .into_iter()
                .flatten()
                .map(String::as_str),
        )
    }
}

fn normalize_scopes(scopes_supported: Option<Vec<String>>) -> Option<Vec<String>> {
    let scopes_supported = scopes_supported?;

    let mut normalized = Vec::new();
    for scope in scopes_supported {
        let scope = scope.trim();
        if scope.is_empty() {
            continue;
        }
        let scope = scope.to_string();
        if !normalized.contains(&scope) {
            normalized.push(scope);
        }
    }

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn authorization_metadata_urls(base_url: &Url) -> Vec<Url> {
    let mut urls = Vec::new();
    for candidate_path in authorization_metadata_paths(base_url.path()) {
        let mut discovery_url = base_url.clone();
        discovery_url.set_query(None);
        discovery_url.set_fragment(None);
        discovery_url.set_path(&candidate_path);
        urls.push(discovery_url);
    }
    urls
}

/// Implements RFC 8414 section 3.1 for discovering well-known OAuth endpoints.
/// Also tries OpenID Connect metadata because some OAuth brokers expose the same
/// authorization and token endpoint fields there.
fn authorization_metadata_paths(base_path: &str) -> Vec<String> {
    let trimmed = base_path.trim_start_matches('/').trim_end_matches('/');
    let canonical_oauth = "/.well-known/oauth-authorization-server".to_string();
    let canonical_openid = "/.well-known/openid-configuration".to_string();

    if trimmed.is_empty() {
        return vec![canonical_oauth, canonical_openid];
    }

    let mut candidates = Vec::new();
    let mut push_unique = |candidate: String| {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    };

    push_unique(format!("{canonical_oauth}/{trimmed}"));
    push_unique(format!("{canonical_openid}/{trimmed}"));
    push_unique(format!("/{trimmed}/.well-known/oauth-authorization-server"));
    push_unique(format!("/{trimmed}/.well-known/openid-configuration"));
    push_unique(canonical_oauth);
    push_unique(canonical_openid);

    candidates
}

fn protected_resource_metadata_urls(base_url: &Url) -> Vec<Url> {
    let mut urls = Vec::new();
    for candidate_path in protected_resource_metadata_paths(base_url.path()) {
        let mut discovery_url = base_url.clone();
        discovery_url.set_query(None);
        discovery_url.set_fragment(None);
        discovery_url.set_path(&candidate_path);
        urls.push(discovery_url);
    }
    urls
}

fn protected_resource_metadata_paths(base_path: &str) -> Vec<String> {
    let trimmed = base_path.trim_start_matches('/').trim_end_matches('/');
    let canonical = "/.well-known/oauth-protected-resource".to_string();

    if trimmed.is_empty() {
        return vec![canonical];
    }

    let mut candidates = Vec::new();
    let mut push_unique = |candidate: String| {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    };

    push_unique(format!("{canonical}/{trimmed}"));
    push_unique(format!("/{trimmed}/.well-known/oauth-protected-resource"));
    push_unique(canonical);

    candidates
}

fn extract_resource_metadata_url_from_header(header: &str, base_url: &Url) -> Option<Url> {
    let header_lowercase = header.to_ascii_lowercase();
    let fragment_key = "resource_metadata=";
    let mut search_offset = 0;

    while let Some(pos) = header_lowercase[search_offset..].find(fragment_key) {
        let global_pos = search_offset + pos + fragment_key.len();
        let value_slice = &header[global_pos..];
        let Some((value, consumed)) = parse_next_header_value(value_slice) else {
            break;
        };

        if let Ok(url) = Url::parse(&value) {
            return Some(url);
        }
        if let Ok(url) = base_url.join(&value) {
            return Some(url);
        }

        debug!("failed to parse resource metadata value `{value}` as URL");
        search_offset = global_pos + consumed;
    }

    None
}

fn parse_next_header_value(header_fragment: &str) -> Option<(String, usize)> {
    let trimmed = header_fragment.trim_start();
    let leading_ws = header_fragment.len() - trimmed.len();

    if let Some(stripped) = trimmed.strip_prefix('"') {
        let mut escaped = false;
        let mut result = String::new();
        for (idx, ch) in stripped.char_indices() {
            if escaped {
                result.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => return Some((result, leading_ws + idx + 2)),
                _ => result.push(ch),
            }
        }
        None
    } else {
        let end = trimmed
            .find(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .unwrap_or(trimmed.len());
        Some((trimmed[..end].to_string(), leading_ws + end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Json;
    use axum::Router;
    use axum::http::HeaderMap as AxumHeaderMap;
    use axum::http::HeaderValue;
    use axum::http::StatusCode as AxumStatusCode;
    use axum::http::header::WWW_AUTHENTICATE as AXUM_WWW_AUTHENTICATE;
    use axum::routing::get;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use tokio::task::JoinHandle;

    struct TestServer {
        url: String,
        handle: JoinHandle<()>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    async fn spawn_oauth_discovery_server(metadata: serde_json::Value) -> TestServer {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let app = Router::new().route(
            "/.well-known/oauth-authorization-server/mcp",
            get({
                let metadata = metadata.clone();
                move || {
                    let metadata = metadata.clone();
                    async move { Json(metadata) }
                }
            }),
        );
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });

        TestServer {
            url: format!("http://{address}/mcp"),
            handle,
        }
    }

    async fn spawn_server(app: Router, path: &str) -> TestServer {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });

        TestServer {
            url: format!("http://{address}{path}"),
            handle,
        }
    }

    struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: &str) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self {
                key: key.to_string(),
                original,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                unsafe {
                    std::env::set_var(&self.key, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(&self.key);
                }
            }
        }
    }

    #[tokio::test]
    async fn determine_auth_status_uses_bearer_token_when_authorization_header_present() {
        let status = determine_streamable_http_auth_status(
            "server",
            "not-a-url",
            /*bearer_token_env_var*/ None,
            Some(HashMap::from([(
                "Authorization".to_string(),
                "Bearer token".to_string(),
            )])),
            /*env_http_headers*/ None,
            OAuthCredentialsStoreMode::Keyring,
        )
        .await
        .expect("status should compute");

        assert_eq!(status, McpAuthStatus::BearerToken);
    }

    #[tokio::test]
    #[serial(auth_status_env)]
    async fn determine_auth_status_uses_bearer_token_when_env_authorization_header_present() {
        let _guard = EnvVarGuard::set("CODEX_RMCP_CLIENT_AUTH_STATUS_TEST_TOKEN", "Bearer token");
        let status = determine_streamable_http_auth_status(
            "server",
            "not-a-url",
            /*bearer_token_env_var*/ None,
            /*http_headers*/ None,
            Some(HashMap::from([(
                "Authorization".to_string(),
                "CODEX_RMCP_CLIENT_AUTH_STATUS_TEST_TOKEN".to_string(),
            )])),
            OAuthCredentialsStoreMode::Keyring,
        )
        .await
        .expect("status should compute");

        assert_eq!(status, McpAuthStatus::BearerToken);
    }

    #[tokio::test]
    async fn discover_streamable_http_oauth_returns_normalized_scopes() {
        let server = spawn_oauth_discovery_server(serde_json::json!({
            "authorization_endpoint": "https://example.com/authorize",
            "token_endpoint": "https://example.com/token",
            "scopes_supported": ["profile", " email ", "profile", "", "   "],
        }))
        .await;

        let discovery = discover_streamable_http_oauth(
            &server.url,
            /*http_headers*/ None,
            /*env_http_headers*/ None,
        )
        .await
        .expect("discovery should succeed")
        .expect("oauth support should be detected");

        assert_eq!(
            discovery.scopes_supported,
            Some(vec!["profile".to_string(), "email".to_string()])
        );
    }

    #[tokio::test]
    async fn discover_streamable_http_oauth_uses_protected_resource_header() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let resource_metadata_url = format!("http://{address}/resource-metadata");
        let authorization_server_url = format!("http://{address}/broker");

        let app = Router::new()
            .route(
                "/mcp",
                get({
                    let resource_metadata_url = resource_metadata_url.clone();
                    move || {
                        let resource_metadata_url = resource_metadata_url.clone();
                        async move {
                            let mut headers = AxumHeaderMap::new();
                            headers.insert(
                                AXUM_WWW_AUTHENTICATE,
                                HeaderValue::from_str(&format!(
                                    "Bearer resource_metadata=\"{resource_metadata_url}\""
                                ))
                                .expect("header should be valid"),
                            );
                            (AxumStatusCode::UNAUTHORIZED, headers)
                        }
                    }
                }),
            )
            .route(
                "/resource-metadata",
                get({
                    let authorization_server_url = authorization_server_url.clone();
                    move || {
                        let authorization_server_url = authorization_server_url.clone();
                        async move {
                            Json(serde_json::json!({
                                "authorization_servers": [authorization_server_url],
                            }))
                        }
                    }
                }),
            )
            .route(
                "/broker/.well-known/oauth-authorization-server",
                get(|| async {
                    Json(serde_json::json!({
                        "authorization_endpoint": "https://broker.example.com/authorize",
                        "token_endpoint": "https://broker.example.com/token",
                        "scopes_supported": ["maas.read"],
                    }))
                }),
            );
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });
        let server = TestServer {
            url: format!("http://{address}/mcp"),
            handle,
        };

        let discovery = discover_streamable_http_oauth(
            &server.url,
            /*http_headers*/ None,
            /*env_http_headers*/ None,
        )
        .await
        .expect("discovery should succeed")
        .expect("oauth support should be detected through protected resource metadata");

        assert_eq!(discovery.scopes_supported, Some(vec!["maas.read".into()]));
    }

    #[tokio::test]
    async fn discover_streamable_http_oauth_uses_protected_resource_well_known() {
        let app = Router::new()
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                get(|| async {
                    Json(serde_json::json!({
                        "authorization_server": "/broker/.well-known/oauth-authorization-server",
                    }))
                }),
            )
            .route(
                "/broker/.well-known/oauth-authorization-server",
                get(|| async {
                    Json(serde_json::json!({
                        "authorization_endpoint": "https://broker.example.com/authorize",
                        "token_endpoint": "https://broker.example.com/token",
                    }))
                }),
            );
        let server = spawn_server(app, "/mcp").await;

        let discovery = discover_streamable_http_oauth(
            &server.url,
            /*http_headers*/ None,
            /*env_http_headers*/ None,
        )
        .await
        .expect("discovery should succeed")
        .expect("oauth support should be detected through protected resource well-known metadata");

        assert_eq!(discovery.scopes_supported, None);
    }

    #[tokio::test]
    async fn discover_streamable_http_oauth_ignores_empty_scopes() {
        let server = spawn_oauth_discovery_server(serde_json::json!({
            "authorization_endpoint": "https://example.com/authorize",
            "token_endpoint": "https://example.com/token",
            "scopes_supported": ["", "   "],
        }))
        .await;

        let discovery = discover_streamable_http_oauth(
            &server.url,
            /*http_headers*/ None,
            /*env_http_headers*/ None,
        )
        .await
        .expect("discovery should succeed")
        .expect("oauth support should be detected");

        assert_eq!(discovery.scopes_supported, None);
    }

    #[tokio::test]
    async fn supports_oauth_login_does_not_require_scopes_supported() {
        let server = spawn_oauth_discovery_server(serde_json::json!({
            "authorization_endpoint": "https://example.com/authorize",
            "token_endpoint": "https://example.com/token",
        }))
        .await;

        let supported = supports_oauth_login(&server.url)
            .await
            .expect("support check should succeed");

        assert!(supported);
    }
}
