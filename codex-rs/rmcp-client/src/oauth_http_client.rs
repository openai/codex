//! OAuth bootstrap helpers that route non-browser HTTP through the shared
//! `HttpClient` capability.
//!
//! The browser-facing part of MCP OAuth stays on the orchestrator, but the
//! HTTP requests used for discovery, registration, token exchange, and refresh
//! need to follow the selected MCP placement. This module discovers OAuth
//! metadata through `HttpClient` and exposes token/registration endpoints
//! through a small localhost proxy so RMCP's OAuth manager can keep using its
//! existing `reqwest`-based internals.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::thread;

use anyhow::Result;
use anyhow::anyhow;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestParams;
use reqwest::Method;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header::HeaderMap;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::CredentialStore;
use rmcp::transport::auth::StoredCredentials;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server;

const DISCOVERY_TIMEOUT_MS: u64 = 30_000;
const MCP_PROTOCOL_VERSION_HEADER: &str = "MCP-Protocol-Version";
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const TOKEN_PROXY_PATH: &str = "/oauth/token";
const REGISTRATION_PROXY_PATH: &str = "/oauth/register";

pub(crate) struct OAuthHttpSetup {
    pub(crate) authorization_manager: AuthorizationManager,
    pub(crate) proxy: Option<OAuthHttpProxy>,
}

pub(crate) async fn create_oauth_http_setup(
    url: &str,
    default_headers: &HeaderMap,
    http_client: Arc<dyn HttpClient>,
    initial_credentials: Option<StoredCredentials>,
) -> Result<OAuthHttpSetup> {
    let metadata = discover_authorization_metadata(url, default_headers, Arc::clone(&http_client))
        .await
        .map_err(|error| anyhow!(error))?;
    let proxy = OAuthHttpProxy::new(Arc::clone(&http_client), default_headers, &metadata)?;

    let mut authorization_manager = AuthorizationManager::new(url)
        .await
        .map_err(|error| anyhow!(error))?;
    authorization_manager.set_metadata(proxy.rewrite_metadata(&metadata)?);

    if let Some(initial_credentials) = initial_credentials {
        let client_id = initial_credentials.client_id.clone();
        authorization_manager.set_credential_store(StaticCredentialStore::new(initial_credentials));
        authorization_manager
            .configure_client_id(&client_id)
            .map_err(|error| anyhow!(error))?;
    }

    Ok(OAuthHttpSetup {
        authorization_manager,
        proxy: Some(proxy),
    })
}

async fn discover_authorization_metadata(
    url: &str,
    default_headers: &HeaderMap,
    http_client: Arc<dyn HttpClient>,
) -> Result<AuthorizationMetadata, AuthError> {
    let base_url = Url::parse(url).map_err(|error| AuthError::OAuthError(error.to_string()))?;

    for discovery_url in generate_discovery_urls(&base_url) {
        let response = http_client
            .http_request(HttpRequestParams {
                method: Method::GET.as_str().to_string(),
                url: discovery_url.to_string(),
                headers: discovery_headers(default_headers)?,
                body: None,
                timeout_ms: Some(DISCOVERY_TIMEOUT_MS),
                request_id: next_request_id("oauth-discovery"),
                stream_response: false,
            })
            .await
            .map_err(|error| AuthError::OAuthError(error.to_string()))?;

        if response.status != StatusCode::OK.as_u16() {
            continue;
        }

        let metadata = serde_json::from_slice::<AuthorizationMetadata>(&response.body.into_inner())
            .map_err(|error| AuthError::OAuthError(error.to_string()))?;
        return Ok(metadata);
    }

    Err(AuthError::NoAuthorizationSupport)
}

fn generate_discovery_urls(base_url: &Url) -> Vec<Url> {
    let mut candidates = Vec::new();
    let trimmed = base_url
        .path()
        .trim_start_matches('/')
        .trim_end_matches('/');
    let mut push_candidate = |discovery_path: String| {
        let mut discovery_url = base_url.clone();
        discovery_url.set_query(None);
        discovery_url.set_fragment(None);
        discovery_url.set_path(&discovery_path);
        candidates.push(discovery_url);
    };

    if trimmed.is_empty() {
        push_candidate("/.well-known/oauth-authorization-server".to_string());
        push_candidate("/.well-known/openid-configuration".to_string());
    } else {
        push_candidate(format!("/.well-known/oauth-authorization-server/{trimmed}"));
        push_candidate(format!("/.well-known/openid-configuration/{trimmed}"));
        push_candidate(format!("/{trimmed}/.well-known/openid-configuration"));
        push_candidate("/.well-known/oauth-authorization-server".to_string());
    }

    candidates
}

fn discovery_headers(default_headers: &HeaderMap) -> Result<Vec<HttpHeader>, AuthError> {
    let mut headers = protocol_headers(default_headers);
    headers.push(HttpHeader {
        name: MCP_PROTOCOL_VERSION_HEADER.to_string(),
        value: MCP_PROTOCOL_VERSION.to_string(),
    });
    Ok(headers)
}

pub(crate) struct OAuthHttpProxy {
    server: Arc<Server>,
    base_url: String,
    routes: Arc<HashMap<String, String>>,
}

impl OAuthHttpProxy {
    fn new(
        http_client: Arc<dyn HttpClient>,
        default_headers: &HeaderMap,
        metadata: &AuthorizationMetadata,
    ) -> Result<Self> {
        let server = Arc::new(Server::http("127.0.0.1:0").map_err(|error| anyhow!(error))?);
        let address = match server.server_addr() {
            tiny_http::ListenAddr::IP(address) => address,
            #[cfg(not(target_os = "windows"))]
            _ => return Err(anyhow!("unable to determine OAuth HTTP proxy bind address")),
        };
        let base_url = format!("http://{address}");

        let mut routes = HashMap::new();
        routes.insert(
            TOKEN_PROXY_PATH.to_string(),
            metadata.token_endpoint.clone(),
        );
        if let Some(registration_endpoint) = metadata.registration_endpoint.clone() {
            routes.insert(REGISTRATION_PROXY_PATH.to_string(), registration_endpoint);
        }
        let routes = Arc::new(routes);

        let server_for_thread = Arc::clone(&server);
        let routes_for_thread = Arc::clone(&routes);
        let default_headers = default_headers.clone();
        let runtime = tokio::runtime::Handle::current();
        thread::spawn(move || {
            while let Ok(mut request) = server_for_thread.recv() {
                let request_url = request.url().to_string();
                let route_key = request_url
                    .split_once('?')
                    .map(|(path, _)| path)
                    .unwrap_or(request_url.as_str())
                    .to_string();
                let Some(target_url) = routes_for_thread.get(&route_key).cloned() else {
                    let _ = request.respond(Response::empty(StatusCode::NOT_FOUND.as_u16()));
                    continue;
                };

                let mut target = target_url;
                if let Some((_, query)) = request_url.split_once('?') {
                    target.push('?');
                    target.push_str(query);
                }

                let method = request.method().as_str().to_string();
                let mut body = Vec::new();
                if request.as_reader().read_to_end(&mut body).is_err() {
                    let _ = request.respond(Response::empty(StatusCode::BAD_REQUEST.as_u16()));
                    continue;
                }

                let mut headers = protocol_headers(&default_headers);
                for header in request.headers() {
                    let field = header.field.as_str().to_string();
                    if field.eq_ignore_ascii_case("host")
                        || field.eq_ignore_ascii_case("content-length")
                    {
                        continue;
                    }
                    headers.push(HttpHeader {
                        name: field,
                        value: header.value.to_string(),
                    });
                }

                let response = runtime.block_on(http_client.http_request(HttpRequestParams {
                    method,
                    url: target,
                    headers,
                    body: Some(body.into()),
                    timeout_ms: Some(DISCOVERY_TIMEOUT_MS),
                    request_id: next_request_id("oauth-proxy"),
                    stream_response: false,
                }));

                let response = match response {
                    Ok(response) => response,
                    Err(_) => {
                        let _ = request.respond(Response::empty(StatusCode::BAD_GATEWAY.as_u16()));
                        continue;
                    }
                };

                let mut proxy_response = Response::from_data(response.body.into_inner())
                    .with_status_code(response.status);
                for header in response.headers {
                    if let Ok(header) = Header::from_bytes(header.name.as_bytes(), header.value) {
                        proxy_response.add_header(header);
                    }
                }
                let _ = request.respond(proxy_response);
            }
        });

        Ok(Self {
            server,
            base_url,
            routes,
        })
    }

    fn rewrite_metadata(&self, metadata: &AuthorizationMetadata) -> Result<AuthorizationMetadata> {
        let mut rewritten = metadata.clone();
        rewritten.token_endpoint = self.proxied_url(TOKEN_PROXY_PATH)?;
        rewritten.registration_endpoint = if self.routes.contains_key(REGISTRATION_PROXY_PATH) {
            Some(self.proxied_url(REGISTRATION_PROXY_PATH)?)
        } else {
            None
        };
        Ok(rewritten)
    }

    fn proxied_url(&self, path: &str) -> Result<String> {
        let mut url = Url::parse(&self.base_url).map_err(|error| anyhow!(error))?;
        url.set_path(path);
        Ok(url.to_string())
    }
}

impl Drop for OAuthHttpProxy {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

#[derive(Clone)]
struct StaticCredentialStore {
    stored: Arc<tokio::sync::Mutex<Option<StoredCredentials>>>,
}

impl StaticCredentialStore {
    fn new(credentials: StoredCredentials) -> Self {
        Self {
            stored: Arc::new(tokio::sync::Mutex::new(Some(credentials))),
        }
    }
}

#[async_trait::async_trait]
impl CredentialStore for StaticCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        Ok(self.stored.lock().await.clone())
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        *self.stored.lock().await = Some(credentials);
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        *self.stored.lock().await = None;
        Ok(())
    }
}

fn protocol_headers(headers: &HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some(HttpHeader {
                name: name.as_str().to_string(),
                value: value.to_str().ok()?.to_string(),
            })
        })
        .collect()
}

fn next_request_id(prefix: &str) -> String {
    static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{id}")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::Result;
    use codex_exec_server::ExecServerError;
    use codex_exec_server::HttpRequestResponse;
    use futures::FutureExt;
    use futures::future::BoxFuture;
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, Default)]
    struct TestHttpClient {
        requests: Arc<Mutex<Vec<HttpRequestParams>>>,
        responses: Arc<Mutex<Vec<Result<HttpRequestResponse, ExecServerError>>>>,
    }

    impl TestHttpClient {
        fn push_response(&self, response: Result<HttpRequestResponse, ExecServerError>) {
            self.responses.lock().unwrap().push(response);
        }

        fn requests(&self) -> Vec<HttpRequestParams> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl HttpClient for TestHttpClient {
        fn http_request(
            &self,
            params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
            let requests = Arc::clone(&self.requests);
            let responses = Arc::clone(&self.responses);
            async move {
                requests.lock().unwrap().push(params);
                responses.lock().unwrap().remove(0)
            }
            .boxed()
        }

        fn http_request_stream(
            &self,
            _params: HttpRequestParams,
        ) -> BoxFuture<
            '_,
            Result<
                (
                    codex_exec_server::HttpRequestResponse,
                    codex_exec_server::HttpResponseBodyStream,
                ),
                ExecServerError,
            >,
        > {
            async move { Err(ExecServerError::Protocol("unused".to_string())) }.boxed()
        }
    }

    #[tokio::test]
    async fn discover_authorization_metadata_uses_http_client() -> Result<()> {
        let http_client = TestHttpClient::default();
        http_client.push_response(Ok(HttpRequestResponse {
            status: StatusCode::OK.as_u16(),
            headers: Vec::new(),
            body: serde_json::to_vec(&AuthorizationMetadata {
                authorization_endpoint: "https://example.com/authorize".to_string(),
                token_endpoint: "https://example.com/token".to_string(),
                registration_endpoint: None,
                issuer: None,
                jwks_uri: None,
                scopes_supported: None,
                response_types_supported: None,
                additional_fields: HashMap::new(),
            })?
            .into(),
        }));

        let metadata = discover_authorization_metadata(
            "https://example.com/mcp",
            &HeaderMap::new(),
            Arc::new(http_client.clone()),
        )
        .await?;

        assert_eq!(
            metadata.authorization_endpoint,
            "https://example.com/authorize"
        );
        assert_eq!(metadata.token_endpoint, "https://example.com/token");
        assert_eq!(
            http_client.requests()[0].url,
            "https://example.com/.well-known/oauth-authorization-server/mcp"
        );
        Ok(())
    }

    #[tokio::test]
    async fn oauth_http_proxy_forwards_token_requests() -> Result<()> {
        let http_client = TestHttpClient::default();
        http_client.push_response(Ok(HttpRequestResponse {
            status: StatusCode::OK.as_u16(),
            headers: vec![HttpHeader {
                name: "content-type".to_string(),
                value: "application/json".to_string(),
            }],
            body: br#"{"access_token":"abc","token_type":"Bearer"}"#.to_vec().into(),
        }));

        let proxy = OAuthHttpProxy::new(
            Arc::new(http_client.clone()),
            &HeaderMap::new(),
            &AuthorizationMetadata {
                authorization_endpoint: "https://example.com/authorize".to_string(),
                token_endpoint: "https://remote.example.com/token".to_string(),
                registration_endpoint: None,
                issuer: None,
                jwks_uri: None,
                scopes_supported: None,
                response_types_supported: None,
                additional_fields: HashMap::new(),
            },
        )?;

        let response = reqwest::Client::new()
            .post(proxy.proxied_url(TOKEN_PROXY_PATH)?)
            .header("content-type", "application/x-www-form-urlencoded")
            .body("grant_type=refresh_token")
            .send()
            .await?;

        assert_eq!(response.status(), StatusCode::OK);

        let requests = http_client.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, "https://remote.example.com/token");
        assert_eq!(
            String::from_utf8(requests[0].body.clone().unwrap().into_inner())?,
            "grant_type=refresh_token"
        );
        Ok(())
    }
}
