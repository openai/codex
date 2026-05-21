use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::ReqwestHttpClient;
use codex_protocol::protocol::McpAuthStatus;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderMap;
use reqwest::header::WWW_AUTHENTICATE;
use serde::Deserialize;
use tracing::debug;

use crate::oauth::has_oauth_tokens;
use crate::utils::build_default_headers;
use crate::utils::oauth_discovery_headers;
use codex_config::types::OAuthCredentialsStoreMode;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const OAUTH_DISCOVERY_HEADER: &str = "MCP-Protocol-Version";
const OAUTH_DISCOVERY_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamableHttpOAuthDiscovery {
    pub scopes_supported: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StreamableHttpOAuthMetadata {
    pub(crate) authorization_endpoint: String,
    pub(crate) token_endpoint: String,
    pub(crate) registration_endpoint: Option<String>,
    pub(crate) scopes_supported: Option<Vec<String>>,
}

/// Determine the authentication status for a streamable HTTP MCP server.
pub async fn determine_streamable_http_auth_status(
    server_name: &str,
    url: &str,
    bearer_token_env_var: Option<&str>,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    store_mode: OAuthCredentialsStoreMode,
    http_client: Arc<dyn HttpClient>,
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

    match discover_streamable_http_oauth_with_headers(url, &default_headers, http_client).await {
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
    supports_oauth_login_with_http_client(url, Arc::new(ReqwestHttpClient)).await
}

pub async fn supports_oauth_login_with_http_client(
    url: &str,
    http_client: Arc<dyn HttpClient>,
) -> Result<bool> {
    Ok(discover_streamable_http_oauth_with_http_client(
        url,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        http_client,
    )
    .await?
    .is_some())
}

pub async fn discover_streamable_http_oauth(
    url: &str,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
) -> Result<Option<StreamableHttpOAuthDiscovery>> {
    discover_streamable_http_oauth_with_http_client(
        url,
        http_headers,
        env_http_headers,
        Arc::new(ReqwestHttpClient),
    )
    .await
}

pub async fn discover_streamable_http_oauth_with_http_client(
    url: &str,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<StreamableHttpOAuthDiscovery>> {
    Ok(
        discover_streamable_http_oauth_metadata(url, http_headers, env_http_headers, http_client)
            .await?
            .map(|metadata| StreamableHttpOAuthDiscovery {
                scopes_supported: metadata.scopes_supported,
            }),
    )
}

pub(crate) async fn discover_streamable_http_oauth_metadata(
    url: &str,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<StreamableHttpOAuthMetadata>> {
    let default_headers = build_default_headers(http_headers, env_http_headers)?;
    discover_streamable_http_oauth_with_headers(url, &default_headers, http_client).await
}

async fn discover_streamable_http_oauth_with_headers(
    url: &str,
    default_headers: &HeaderMap,
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<StreamableHttpOAuthMetadata>> {
    let base_url = Url::parse(url)?;
    let request_headers = oauth_request_headers(default_headers)?;
    if let Some(metadata) =
        try_discover_oauth_server(&base_url, &request_headers, Arc::clone(&http_client)).await?
    {
        return Ok(Some(metadata));
    }

    discover_oauth_server_via_resource_metadata(&base_url, &request_headers, http_client).await
}

#[derive(Debug, Deserialize)]
struct OAuthDiscoveryMetadata {
    #[serde(default)]
    authorization_endpoint: Option<String>,
    #[serde(default)]
    token_endpoint: Option<String>,
    #[serde(default)]
    registration_endpoint: Option<String>,
    #[serde(default)]
    scopes_supported: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ResourceServerMetadata {
    #[serde(default)]
    authorization_server: Option<String>,
    #[serde(default)]
    authorization_servers: Option<Vec<String>>,
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

fn oauth_request_headers(
    default_headers: &HeaderMap,
) -> Result<Vec<codex_exec_server::HttpHeader>> {
    let mut request_headers = default_headers.clone();
    request_headers.insert(OAUTH_DISCOVERY_HEADER, OAUTH_DISCOVERY_VERSION.parse()?);
    Ok(oauth_discovery_headers(&request_headers))
}

async fn try_discover_oauth_server(
    base_url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<StreamableHttpOAuthMetadata>> {
    for discovery_url in discovery_urls(base_url) {
        if let Some(metadata) =
            fetch_authorization_metadata(&discovery_url, request_headers, Arc::clone(&http_client))
                .await?
        {
            return Ok(Some(metadata));
        }
    }
    Ok(None)
}

async fn discover_oauth_server_via_resource_metadata(
    base_url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<StreamableHttpOAuthMetadata>> {
    let Some(resource_metadata_url) =
        discover_resource_metadata_url(base_url, request_headers, Arc::clone(&http_client)).await?
    else {
        return Ok(None);
    };
    let Some(resource_metadata) = fetch_resource_metadata_from_url(
        &resource_metadata_url,
        request_headers,
        Arc::clone(&http_client),
    )
    .await?
    else {
        return Ok(None);
    };

    let candidates = resource_metadata.authorization_server.into_iter().chain(
        resource_metadata
            .authorization_servers
            .into_iter()
            .flatten(),
    );
    for candidate in candidates {
        let candidate = candidate.trim();
        if candidate.is_empty() {
            continue;
        }

        let candidate_url = match Url::parse(candidate) {
            Ok(url) => url,
            Err(_) => match resource_metadata_url.join(candidate) {
                Ok(url) => url,
                Err(err) => {
                    debug!("failed to resolve authorization server URL `{candidate}`: {err}");
                    continue;
                }
            },
        };
        if candidate_url.path().contains("/.well-known/") {
            if let Some(metadata) = fetch_authorization_metadata(
                &candidate_url,
                request_headers,
                Arc::clone(&http_client),
            )
            .await?
            {
                return Ok(Some(metadata));
            }
            continue;
        }

        if let Some(metadata) =
            try_discover_oauth_server(&candidate_url, request_headers, Arc::clone(&http_client))
                .await?
        {
            return Ok(Some(metadata));
        }
    }

    Ok(None)
}

async fn discover_resource_metadata_url(
    base_url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<Url>> {
    if let Some(resource_metadata_url) = fetch_resource_metadata_url(
        base_url,
        base_url,
        request_headers,
        Arc::clone(&http_client),
    )
    .await?
    {
        return Ok(Some(resource_metadata_url));
    }

    for candidate_path in well_known_paths(base_url.path(), "oauth-protected-resource") {
        let mut discovery_url = base_url.clone();
        discovery_url.set_query(None);
        discovery_url.set_fragment(None);
        discovery_url.set_path(&candidate_path);
        if let Some(resource_metadata_url) = fetch_resource_metadata_url(
            &discovery_url,
            base_url,
            request_headers,
            Arc::clone(&http_client),
        )
        .await?
        {
            return Ok(Some(resource_metadata_url));
        }
    }

    Ok(None)
}

async fn fetch_resource_metadata_url(
    url: &Url,
    base_url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<Url>> {
    let response = match oauth_get(url, request_headers, http_client).await {
        Ok(response) => response,
        Err(err) => {
            debug!("resource metadata probe failed for {url}: {err:?}");
            return Ok(None);
        }
    };

    if response.status == StatusCode::OK.as_u16() {
        return Ok(Some(url.clone()));
    }
    if response.status != StatusCode::UNAUTHORIZED.as_u16() {
        return Ok(None);
    }

    Ok(response
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case(WWW_AUTHENTICATE.as_str()))
        .find_map(|header| extract_resource_metadata_url_from_header(&header.value, base_url)))
}

async fn fetch_resource_metadata_from_url(
    resource_metadata_url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<ResourceServerMetadata>> {
    let response = match oauth_get(resource_metadata_url, request_headers, http_client).await {
        Ok(response) => response,
        Err(err) => {
            debug!("resource metadata request failed for {resource_metadata_url}: {err:?}");
            return Ok(None);
        }
    };
    if response.status != StatusCode::OK.as_u16() {
        return Ok(None);
    }

    Ok(Some(serde_json::from_slice(&response.body.into_inner())?))
}

async fn fetch_authorization_metadata(
    discovery_url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<StreamableHttpOAuthMetadata>> {
    let response = match oauth_get(discovery_url, request_headers, http_client).await {
        Ok(response) => response,
        Err(err) => {
            debug!("OAuth discovery request failed for {discovery_url}: {err:?}");
            return Ok(None);
        }
    };
    if response.status != StatusCode::OK.as_u16() {
        return Ok(None);
    }

    let metadata =
        match serde_json::from_slice::<OAuthDiscoveryMetadata>(&response.body.into_inner()) {
            Ok(metadata) => metadata,
            Err(err) => {
                debug!("failed to parse OAuth metadata for {discovery_url}: {err}");
                return Ok(None);
            }
        };
    let (Some(authorization_endpoint), Some(token_endpoint)) =
        (metadata.authorization_endpoint, metadata.token_endpoint)
    else {
        return Ok(None);
    };

    Ok(Some(StreamableHttpOAuthMetadata {
        authorization_endpoint,
        token_endpoint,
        registration_endpoint: metadata.registration_endpoint,
        scopes_supported: normalize_scopes(metadata.scopes_supported),
    }))
}

async fn oauth_get(
    url: &Url,
    request_headers: &[codex_exec_server::HttpHeader],
    http_client: Arc<dyn HttpClient>,
) -> Result<codex_exec_server::HttpRequestResponse> {
    http_client
        .http_request(HttpRequestParams {
            method: "GET".to_string(),
            url: url.to_string(),
            headers: request_headers.to_vec(),
            body: None,
            timeout_ms: Some(DISCOVERY_TIMEOUT.as_millis() as u64),
            request_id: "oauth-discovery".to_string(),
            stream_response: false,
        })
        .await
        .map_err(Into::into)
}

/// Implements MCP authorization server metadata discovery priority.
fn discovery_urls(base_url: &Url) -> Vec<Url> {
    discovery_paths(base_url.path())
        .into_iter()
        .map(|candidate_path| {
            let mut discovery_url = base_url.clone();
            discovery_url.set_query(None);
            discovery_url.set_fragment(None);
            discovery_url.set_path(&candidate_path);
            discovery_url
        })
        .collect()
}

fn discovery_paths(base_path: &str) -> Vec<String> {
    let trimmed = base_path.trim_start_matches('/').trim_end_matches('/');
    if trimmed.is_empty() {
        return vec![
            "/.well-known/oauth-authorization-server".to_string(),
            "/.well-known/openid-configuration".to_string(),
        ];
    }
    vec![
        format!("/.well-known/oauth-authorization-server/{trimmed}"),
        format!("/.well-known/openid-configuration/{trimmed}"),
        format!("/{trimmed}/.well-known/openid-configuration"),
        "/.well-known/oauth-authorization-server".to_string(),
    ]
}

fn well_known_paths(base_path: &str, suffix: &str) -> Vec<String> {
    let trimmed = base_path.trim_start_matches('/').trim_end_matches('/');
    let canonical = format!("/.well-known/{suffix}");
    if trimmed.is_empty() {
        return vec![canonical];
    }
    vec![
        format!("{canonical}/{trimmed}"),
        format!("/{trimmed}/.well-known/{suffix}"),
        canonical,
    ]
}

fn extract_resource_metadata_url_from_header(header: &str, base_url: &Url) -> Option<Url> {
    let header_lowercase = header.to_ascii_lowercase();
    let fragment_key = "resource_metadata=";
    let mut search_offset = 0;
    while let Some(position) = header_lowercase[search_offset..].find(fragment_key) {
        let value_offset = search_offset + position + fragment_key.len();
        let value_slice = &header[value_offset..];
        if let Some((value, consumed)) = parse_next_header_value(value_slice) {
            if let Ok(url) = Url::parse(&value) {
                return Some(url);
            }
            if let Ok(url) = base_url.join(&value) {
                return Some(url);
            }
            search_offset = value_offset + consumed;
            continue;
        }
        break;
    }
    None
}

fn parse_next_header_value(header_fragment: &str) -> Option<(String, usize)> {
    let trimmed = header_fragment.trim_start();
    let leading_ws = header_fragment.len() - trimmed.len();
    if let Some(stripped) = trimmed.strip_prefix('"') {
        let mut escaped = false;
        let mut result = String::new();
        for (index, character) in stripped.char_indices() {
            if escaped {
                result.push(character);
                escaped = false;
                continue;
            }
            match character {
                '\\' => escaped = true,
                '"' => return Some((result, leading_ws + index + 2)),
                _ => result.push(character),
            }
        }
        return None;
    }

    let end = trimmed
        .find(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .unwrap_or(trimmed.len());
    Some((trimmed[..end].to_string(), leading_ws + end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Json;
    use axum::Router;
    use axum::routing::get;
    use codex_exec_server::ExecServerError;
    use codex_exec_server::HTTP_REQUEST_NO_PROXY_HEADER;
    use codex_exec_server::HttpClient;
    use codex_exec_server::HttpRequestParams;
    use codex_exec_server::HttpRequestResponse;
    use codex_exec_server::HttpResponseBodyStream;
    use codex_exec_server::ReqwestHttpClient;
    use futures::FutureExt;
    use futures::future::BoxFuture;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use tokio::task::JoinHandle;

    #[derive(Default)]
    struct ScriptedHttpClient {
        requests: std::sync::Mutex<Vec<HttpRequestParams>>,
    }

    impl ScriptedHttpClient {
        fn requests(&self) -> Vec<HttpRequestParams> {
            self.requests.lock().expect("lock requests").clone()
        }
    }

    impl HttpClient for ScriptedHttpClient {
        fn http_request(
            &self,
            params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
            self.requests
                .lock()
                .expect("lock requests")
                .push(params.clone());
            async move {
                let response = match params.url.as_str() {
                    "http://remote-only.invalid/.well-known/oauth-authorization-server/mcp" => {
                        HttpRequestResponse {
                            status: 404,
                            headers: Vec::new(),
                            body: Vec::new().into(),
                        }
                    }
                    "http://remote-only.invalid/.well-known/openid-configuration/mcp" => {
                        HttpRequestResponse {
                            status: 200,
                            headers: Vec::new(),
                            body: serde_json::to_vec(&serde_json::json!({
                                "authorization_endpoint": "https://auth.remote.example/oauth/authorize",
                                "token_endpoint": "https://auth.remote.example/oauth/token",
                            }))
                            .expect("serialize oidc metadata")
                            .into(),
                        }
                    }
                    "http://resource-only.invalid/mcp" => HttpRequestResponse {
                        status: 401,
                        headers: vec![codex_exec_server::HttpHeader {
                            name: "www-authenticate".to_string(),
                            value: "Bearer resource_metadata=\"/.well-known/oauth-protected-resource/mcp\"".to_string(),
                        }],
                        body: Vec::new().into(),
                    },
                    "http://resource-only.invalid/.well-known/oauth-protected-resource/mcp" => {
                        HttpRequestResponse {
                            status: 200,
                            headers: Vec::new(),
                            body: serde_json::to_vec(&serde_json::json!({
                                "authorization_servers": ["https://auth.remote.example/tenant"],
                            }))
                            .expect("serialize resource metadata")
                            .into(),
                        }
                    }
                    "https://auth.remote.example/.well-known/oauth-authorization-server/tenant" => {
                        HttpRequestResponse {
                            status: 200,
                            headers: Vec::new(),
                            body: serde_json::to_vec(&serde_json::json!({
                                "authorization_endpoint": "https://auth.remote.example/oauth/authorize",
                                "token_endpoint": "https://auth.remote.example/oauth/token",
                            }))
                            .expect("serialize oauth metadata")
                            .into(),
                        }
                    }
                    _ => HttpRequestResponse {
                        status: 404,
                        headers: Vec::new(),
                        body: Vec::new().into(),
                    },
                };
                Ok(response)
            }
            .boxed()
        }

        fn http_request_stream(
            &self,
            _params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>>
        {
            async move {
                Err(ExecServerError::HttpRequest(
                    "unexpected stream".to_string(),
                ))
            }
            .boxed()
        }
    }

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
            Arc::new(ReqwestHttpClient),
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
            Arc::new(ReqwestHttpClient),
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

    #[tokio::test]
    async fn discover_streamable_http_oauth_uses_openid_fallback_through_selected_client() {
        let http_client = Arc::new(ScriptedHttpClient::default());
        let discovery = discover_streamable_http_oauth_with_http_client(
            "http://remote-only.invalid/mcp",
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            http_client.clone(),
        )
        .await
        .expect("discovery should succeed")
        .expect("oauth support should be detected");

        assert_eq!(discovery.scopes_supported, None);
        assert_eq!(
            http_client
                .requests()
                .iter()
                .map(|request| request.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "http://remote-only.invalid/.well-known/oauth-authorization-server/mcp",
                "http://remote-only.invalid/.well-known/openid-configuration/mcp",
            ]
        );
        assert!(http_client.requests().iter().all(|request| {
            request
                .headers
                .iter()
                .any(|header| header.name == HTTP_REQUEST_NO_PROXY_HEADER)
        }));
    }

    #[tokio::test]
    async fn discover_streamable_http_oauth_uses_resource_metadata_fallback() {
        let discovery = discover_streamable_http_oauth_with_http_client(
            "http://resource-only.invalid/mcp",
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            Arc::new(ScriptedHttpClient::default()),
        )
        .await
        .expect("discovery should succeed")
        .expect("oauth support should be detected");

        assert_eq!(discovery.scopes_supported, None);
    }
}
