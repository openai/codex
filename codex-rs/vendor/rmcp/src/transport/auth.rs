use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    EmptyExtraTokenFields, HttpRequest, HttpResponse,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, RequestTokenError, Scope,
    StandardTokenResponse, TokenResponse, TokenUrl,
    basic::{BasicClient, BasicTokenType},
};
use reqwest::{
    Client as HttpClient, IntoUrl, StatusCode, Url,
    header::{AUTHORIZATION, WWW_AUTHENTICATE},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, warn};

const DEFAULT_EXCHANGE_URL: &str = "http://localhost";

/// Stored credentials for OAuth2 authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub client_id: String,
    pub token_response: Option<OAuthTokenResponse>,
}

/// Trait for storing and retrieving OAuth2 credentials
///
/// Implementations of this trait can provide custom storage backends
/// for OAuth2 credentials, such as file-based storage, keychain integration,
/// or database storage.
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError>;

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError>;

    async fn clear(&self) -> Result<(), AuthError>;
}

/// In-memory credential store (default implementation)
///
/// This store keeps credentials in memory only and does not persist them
/// between application restarts. This is the default behavior when no
/// custom credential store is provided.
#[derive(Debug, Default, Clone)]
pub struct InMemoryCredentialStore {
    credentials: Arc<RwLock<Option<StoredCredentials>>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl CredentialStore for InMemoryCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        Ok(self.credentials.read().await.clone())
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        *self.credentials.write().await = Some(credentials);
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        *self.credentials.write().await = None;
        Ok(())
    }
}

/// Stored authorization state for OAuth2 PKCE flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuthorizationState {
    pub pkce_verifier: String,
    pub csrf_token: String,
    pub created_at: u64,
}

impl StoredAuthorizationState {
    pub fn new(pkce_verifier: &PkceCodeVerifier, csrf_token: &CsrfToken) -> Self {
        Self {
            pkce_verifier: pkce_verifier.secret().to_string(),
            csrf_token: csrf_token.secret().to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    pub fn into_pkce_verifier(self) -> PkceCodeVerifier {
        PkceCodeVerifier::new(self.pkce_verifier)
    }
}

/// Trait for storing and retrieving OAuth2 authorization state
///
/// Implementations of this trait can provide custom storage backends
/// for OAuth2 PKCE flow state, such as Redis or database storage.
///
/// Implementors are responsible for expiring stale states (e.g., abandoned
/// authorization flows). Use [`StoredAuthorizationState::created_at`] for
/// TTL-based expiration.
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn save(
        &self,
        csrf_token: &str,
        state: StoredAuthorizationState,
    ) -> Result<(), AuthError>;

    async fn load(&self, csrf_token: &str) -> Result<Option<StoredAuthorizationState>, AuthError>;

    async fn delete(&self, csrf_token: &str) -> Result<(), AuthError>;
}

/// HTTP capability used by OAuth discovery, registration, token exchange, and
/// refresh flows.
///
/// Implementations should execute a single HTTP request and return the full
/// response body without applying higher-level OAuth or MCP behavior on top.
#[async_trait]
pub trait OAuthHttpClient: Send + Sync {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, AuthError>;
}

/// In-memory state store (default implementation)
///
/// This store keeps authorization state in memory only and does not persist
/// between application restarts or across multiple server instances.
#[derive(Debug, Default, Clone)]
pub struct InMemoryStateStore {
    states: Arc<RwLock<HashMap<String, StoredAuthorizationState>>>,
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn save(
        &self,
        csrf_token: &str,
        state: StoredAuthorizationState,
    ) -> Result<(), AuthError> {
        self.states
            .write()
            .await
            .insert(csrf_token.to_string(), state);
        Ok(())
    }

    async fn load(&self, csrf_token: &str) -> Result<Option<StoredAuthorizationState>, AuthError> {
        Ok(self.states.read().await.get(csrf_token).cloned())
    }

    async fn delete(&self, csrf_token: &str) -> Result<(), AuthError> {
        self.states.write().await.remove(csrf_token);
        Ok(())
    }
}

/// HTTP client with OAuth 2.0 authorization
#[derive(Clone)]
pub struct AuthClient<C> {
    pub http_client: C,
    pub auth_manager: Arc<Mutex<AuthorizationManager>>,
}

impl<C: std::fmt::Debug> std::fmt::Debug for AuthClient<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizedClient")
            .field("http_client", &self.http_client)
            .field("auth_manager", &"...")
            .finish()
    }
}

impl<C> AuthClient<C> {
    /// Create a new authorized HTTP client
    pub fn new(http_client: C, auth_manager: AuthorizationManager) -> Self {
        Self {
            http_client,
            auth_manager: Arc::new(Mutex::new(auth_manager)),
        }
    }
}

impl<C> AuthClient<C> {
    pub fn get_access_token(&self) -> impl Future<Output = Result<String, AuthError>> + Send {
        let auth_manager = self.auth_manager.clone();
        async move { auth_manager.lock().await.get_access_token().await }
    }
}

/// Auth error
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("OAuth authorization required")]
    AuthorizationRequired,

    #[error("OAuth authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("OAuth token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("OAuth token refresh failed: {0}")]
    TokenRefreshFailed(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("OAuth error: {0}")]
    OAuthError(String),

    #[error("Metadata error: {0}")]
    MetadataError(String),

    #[error("URL parse error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("No authorization support detected")]
    NoAuthorizationSupport,

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Invalid token type: {0}")]
    InvalidTokenType(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid scope: {0}")]
    InvalidScope(String),

    #[error("Registration failed: {0}")]
    RegistrationFailed(String),
}

/// oauth2 metadata
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AuthorizationMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    pub issuer: Option<String>,
    pub jwks_uri: Option<String>,
    pub scopes_supported: Option<Vec<String>>,
    pub response_types_supported: Option<Vec<String>>,
    // allow additional fields
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct ResourceServerMetadata {
    authorization_server: Option<String>,
    authorization_servers: Option<Vec<String>>,
}

/// oauth2 client config
#[derive(Debug, Clone)]
pub struct OAuthClientConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
}

// add type aliases for oauth2 types
type OAuthErrorResponse = oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>;
pub type OAuthTokenResponse = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;
type OAuthTokenIntrospection =
    oauth2::StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>;
type OAuthRevocableToken = oauth2::StandardRevocableToken;
type OAuthRevocationError = oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>;
type OAuthClient = oauth2::Client<
    OAuthErrorResponse,
    OAuthTokenResponse,
    OAuthTokenIntrospection,
    OAuthRevocableToken,
    OAuthRevocationError,
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
>;
type Credentials = (String, Option<OAuthTokenResponse>);

#[derive(Clone)]
enum AuthorizationHttpClient {
    Reqwest(HttpClient),
    Custom(Arc<dyn OAuthHttpClient>),
}

/// oauth2 auth manager
pub struct AuthorizationManager {
    http_client: AuthorizationHttpClient,
    metadata: Option<AuthorizationMetadata>,
    oauth_client: Option<OAuthClient>,
    credential_store: Arc<dyn CredentialStore>,
    state_store: Arc<dyn StateStore>,
    base_url: Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistrationRequest {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub token_endpoint_auth_method: String,
    pub response_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistrationResponse {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub client_name: Option<String>,
    pub redirect_uris: Vec<String>,
    // allow additional fields
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

impl AuthorizationHttpClient {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, AuthError> {
        match self {
            AuthorizationHttpClient::Reqwest(client) => execute_reqwest_request(client, request).await,
            AuthorizationHttpClient::Custom(client) => client.execute(request).await,
        }
    }
}

async fn execute_reqwest_request(
    client: &HttpClient,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    let (parts, body) = request.into_parts();
    let url = parts
        .uri
        .to_string()
        .parse::<Url>()
        .map_err(AuthError::UrlError)?;
    let mut reqwest_request = client.request(parts.method, url).body(body);
    for (name, value) in &parts.headers {
        reqwest_request = reqwest_request.header(name, value);
    }

    let response = reqwest_request.send().await.map_err(AuthError::HttpError)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await.map_err(AuthError::HttpError)?.to_vec();

    let mut response = HttpResponse::new(body);
    *response.status_mut() = status;
    for (name, value) in &headers {
        response.headers_mut().insert(name, value.clone());
    }
    Ok(response)
}

/// SEP-991: URL-based Client IDs
/// Validate that the client_id is a valid URL with https scheme and non-root pathname
fn is_https_url(value: &str) -> bool {
    Url::parse(value)
        .ok()
        .map(|url| url.scheme() == "https" && url.path() != "/" && url.host_str().is_some())
        .unwrap_or(false)
}

impl AuthorizationManager {
    fn well_known_paths(base_path: &str, resource: &str) -> Vec<String> {
        let trimmed = base_path.trim_start_matches('/').trim_end_matches('/');
        let mut candidates = Vec::new();

        let mut push_candidate = |candidate: String| {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        };

        let canonical = format!("/.well-known/{resource}");

        if trimmed.is_empty() {
            push_candidate(canonical);
            return candidates;
        }

        // This follows the RFC 8414 recommendation for well-known URI discovery
        push_candidate(format!("{canonical}/{trimmed}"));
        // This is a common pattern used by some identity providers
        push_candidate(format!("/{trimmed}/.well-known/{resource}"));
        // The canonical path should always be the last fallback
        push_candidate(canonical);

        candidates
    }

    /// create new auth manager with base url
    pub async fn new<U: IntoUrl>(base_url: U) -> Result<Self, AuthError> {
        let base_url = base_url.into_url()?;
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AuthError::InternalError(e.to_string()))?;

        let manager = Self {
            http_client: AuthorizationHttpClient::Reqwest(http_client),
            metadata: None,
            oauth_client: None,
            credential_store: Arc::new(InMemoryCredentialStore::new()),
            state_store: Arc::new(InMemoryStateStore::new()),
            base_url,
        };

        Ok(manager)
    }

    /// Set a custom credential store
    ///
    /// This allows you to provide your own implementation of credential storage,
    /// such as file-based storage, keychain integration, or database storage.
    /// This should be called before any operations that need credentials.
    pub fn set_credential_store<S: CredentialStore + 'static>(&mut self, store: S) {
        self.credential_store = Arc::new(store);
    }

    /// Set a custom state store for OAuth2 authorization flow state
    ///
    /// This should be called before initiating the authorization flow.
    pub fn set_state_store<S: StateStore + 'static>(&mut self, store: S) {
        self.state_store = Arc::new(store);
    }

    /// Set OAuth2 authorization metadata
    ///
    /// This should be called after discovering metadata via `discover_metadata()`
    /// and before creating an `AuthorizationSession`.
    pub fn set_metadata(&mut self, metadata: AuthorizationMetadata) {
        self.metadata = Some(metadata);
    }

    /// Initialize from stored credentials if available
    ///
    /// This will load credentials from the credential store and configure
    /// the client if credentials are found.
    pub async fn initialize_from_store(&mut self) -> Result<bool, AuthError> {
        if let Some(stored) = self.credential_store.load().await? {
            if stored.token_response.is_some() {
                if self.metadata.is_none() {
                    let metadata = self.discover_metadata().await?;
                    self.metadata = Some(metadata);
                }

                self.configure_client_id(&stored.client_id)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn with_client(&mut self, http_client: HttpClient) -> Result<(), AuthError> {
        self.http_client = AuthorizationHttpClient::Reqwest(http_client);
        Ok(())
    }

    pub fn with_oauth_http_client<C>(&mut self, http_client: C) -> Result<(), AuthError>
    where
        C: OAuthHttpClient + 'static,
    {
        self.http_client = AuthorizationHttpClient::Custom(Arc::new(http_client));
        Ok(())
    }

    async fn send_request(&self, request: HttpRequest) -> Result<HttpResponse, AuthError> {
        self.http_client.execute(request).await
    }

    fn build_request(
        &self,
        method: reqwest::Method,
        url: &Url,
        body: Vec<u8>,
        headers: &[(&str, &str)],
    ) -> Result<HttpRequest, AuthError> {
        let mut request = HttpRequest::new(body);
        *request.method_mut() = method;
        let uri = url
            .as_str()
            .parse::<http::Uri>()
            .map_err(|error| AuthError::InternalError(error.to_string()))?;
        *request.uri_mut() = uri;
        for (name, value) in headers {
            let header_name = (*name)
                .parse::<http::HeaderName>()
                .map_err(|error| AuthError::InternalError(error.to_string()))?;
            let header_value = value
                .parse::<http::HeaderValue>()
                .map_err(|error| AuthError::InternalError(error.to_string()))?;
            request.headers_mut().insert(header_name, header_value);
        }
        Ok(request)
    }

    /// discover oauth2 metadata
    pub async fn discover_metadata(&self) -> Result<AuthorizationMetadata, AuthError> {
        if let Some(metadata) = self.try_discover_oauth_server(&self.base_url).await? {
            return Ok(metadata);
        }

        if let Some(metadata) = self.discover_oauth_server_via_resource_metadata().await? {
            return Ok(metadata);
        }

        // No valid authorization metadata found - return error instead of guessing
        // OAuth endpoints must be discovered from the server, not constructed by the client
        Err(AuthError::NoAuthorizationSupport)
    }

    /// get client id and credentials
    pub async fn get_credentials(&self) -> Result<Credentials, AuthError> {
        let client_id = self
            .oauth_client
            .as_ref()
            .ok_or_else(|| AuthError::InternalError("OAuth client not configured".to_string()))?
            .client_id();

        let stored = self.credential_store.load().await?;
        let token_response = stored.and_then(|s| s.token_response);

        Ok((client_id.to_string(), token_response))
    }

    /// configure oauth2 client with client credentials
    pub fn configure_client(&mut self, config: OAuthClientConfig) -> Result<(), AuthError> {
        if self.metadata.is_none() {
            return Err(AuthError::NoAuthorizationSupport);
        }

        let metadata = self.metadata.as_ref().unwrap();

        let auth_url = AuthUrl::new(metadata.authorization_endpoint.clone())
            .map_err(|e| AuthError::OAuthError(format!("Invalid authorization URL: {}", e)))?;

        let token_url = TokenUrl::new(metadata.token_endpoint.clone())
            .map_err(|e| AuthError::OAuthError(format!("Invalid token URL: {}", e)))?;

        let client_id = ClientId::new(config.client_id);
        let redirect_url = RedirectUrl::new(config.redirect_uri.clone())
            .map_err(|e| AuthError::OAuthError(format!("Invalid re URL: {}", e)))?;

        let mut client_builder = BasicClient::new(client_id.clone())
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url);

        if let Some(secret) = config.client_secret {
            client_builder = client_builder.set_client_secret(ClientSecret::new(secret));
        }

        self.oauth_client = Some(client_builder);
        Ok(())
    }
    /// validate if the server support the response type
    fn validate_response_supported(&self, response_type: &str) -> Result<(), AuthError> {
        if let Some(metadata) = self.metadata.as_ref() {
            if let Some(response_types_supported) = metadata.response_types_supported.as_ref() {
                if !response_types_supported.contains(&response_type.to_string()) {
                    return Err(AuthError::InvalidScope(response_type.to_string()));
                }
            }
        }
        Ok(())
    }
    /// dynamic register oauth2 client
    pub async fn register_client(
        &mut self,
        name: &str,
        redirect_uri: &str,
    ) -> Result<OAuthClientConfig, AuthError> {
        if self.metadata.is_none() {
            return Err(AuthError::NoAuthorizationSupport);
        }

        let metadata = self.metadata.as_ref().unwrap();
        let Some(registration_url) = metadata.registration_endpoint.as_ref() else {
            return Err(AuthError::RegistrationFailed(
                "Dynamic client registration not supported".to_string(),
            ));
        };

        // RFC 8414 RECOMMENDS response_types_supported in the metadata. This field is optional,
        // but if present and does not include the flow we use ("code"), bail out early with a clear error.
        self.validate_response_supported("code")?;

        let registration_request = ClientRegistrationRequest {
            client_name: name.to_string(),
            redirect_uris: vec![redirect_uri.to_string()],
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            token_endpoint_auth_method: "none".to_string(), // public client
            response_types: vec!["code".to_string()],
        };

        let registration_url =
            Url::parse(registration_url).map_err(|error| AuthError::UrlError(error))?;
        let body = serde_json::to_vec(&registration_request)
            .map_err(|error| AuthError::RegistrationFailed(error.to_string()))?;
        let response = self
            .send_request(self.build_request(
                reqwest::Method::POST,
                &registration_url,
                body,
                &[("content-type", "application/json")],
            )?)
            .await
            .map_err(|error| AuthError::RegistrationFailed(error.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = String::from_utf8_lossy(response.body()).to_string();

            return Err(AuthError::RegistrationFailed(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        debug!("registration response: {:?}", response);
        let reg_response = match serde_json::from_slice::<ClientRegistrationResponse>(response.body())
        {
            Ok(response) => response,
            Err(e) => {
                return Err(AuthError::RegistrationFailed(format!(
                    "analyze response error: {}",
                    e
                )));
            }
        };

        let config = OAuthClientConfig {
            client_id: reg_response.client_id,
            // Some IdP returns a response where the field 'client_secret' is present but with empty string value.
            // In that case, the interpretation is that the client is a public client and does not have a secret during the
            // registration phase here, e.g. dynamic client registrations.
            //
            // Even though whether or not the empty string is valid is outside of the scope of Oauth2 spec,
            // we should treat it as no secret since otherwise we end up authenticating with a valid client_id with an empty client_secret
            // as a password, which is not a goal of the client secret.
            client_secret: reg_response.client_secret.filter(|s| !s.is_empty()),
            redirect_uri: redirect_uri.to_string(),
            scopes: vec![],
        };

        self.configure_client(config.clone())?;
        Ok(config)
    }

    /// use provided client id to configure oauth2 client instead of dynamic registration
    /// this is useful when you have a stored client id from previous registration
    pub fn configure_client_id(&mut self, client_id: &str) -> Result<(), AuthError> {
        let config = OAuthClientConfig {
            client_id: client_id.to_string(),
            client_secret: None,
            scopes: vec![],
            redirect_uri: self.base_url.to_string(),
        };
        self.configure_client(config)
    }

    /// generate authorization url
    pub async fn get_authorization_url(&self, scopes: &[&str]) -> Result<String, AuthError> {
        let oauth_client = self
            .oauth_client
            .as_ref()
            .ok_or_else(|| AuthError::InternalError("OAuth client not configured".to_string()))?;

        // ensure the server supports the response type we intend to use when metadata is available
        self.validate_response_supported("code")?;

        // generate pkce challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // build authorization request
        let mut auth_request = oauth_client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        // add request scopes
        for scope in scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
        }

        let (auth_url, csrf_token) = auth_request.url();

        // store pkce verifier for later use via state store
        let stored_state = StoredAuthorizationState::new(&pkce_verifier, &csrf_token);
        self.state_store
            .save(csrf_token.secret(), stored_state)
            .await?;

        Ok(auth_url.to_string())
    }

    /// exchange authorization code for access token
    pub async fn exchange_code_for_token(
        &self,
        code: &str,
        csrf_token: &str,
    ) -> Result<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>, AuthError> {
        debug!("start exchange code for token: {:?}", code);
        let oauth_client = self
            .oauth_client
            .as_ref()
            .ok_or_else(|| AuthError::InternalError("OAuth client not configured".to_string()))?;

        // Load state from state store using CSRF token as key
        let stored_state =
            self.state_store.load(csrf_token).await?.ok_or_else(|| {
                AuthError::InternalError("Authorization state not found".to_string())
            })?;

        // Delete state after retrieval (one-time use)
        self.state_store.delete(csrf_token).await?;

        // Reconstruct the PKCE verifier
        let pkce_verifier = stored_state.into_pkce_verifier();

        debug!("client_id: {:?}", oauth_client.client_id());
        let http_client = self.http_client.clone();

        // exchange token
        let token_result = match oauth_client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&move |request| {
                let http_client = http_client.clone();
                async move { http_client.execute(request).await }
            })
            .await
        {
            Ok(token) => token,
            Err(RequestTokenError::Parse(_, body)) => {
                match serde_json::from_slice::<OAuthTokenResponse>(&body) {
                    Ok(parsed) => {
                        warn!(
                            "token exchange failed to parse completely but included a valid token response. Accepting it."
                        );
                        parsed
                    }
                    Err(parse_err) => {
                        return Err(AuthError::TokenExchangeFailed(parse_err.to_string()));
                    }
                }
            }
            Err(e) => {
                return Err(AuthError::TokenExchangeFailed(e.to_string()));
            }
        };

        debug!("exchange token result: {:?}", token_result);

        // Store credentials in the credential store
        let client_id = oauth_client.client_id().to_string();
        let stored = StoredCredentials {
            client_id,
            token_response: Some(token_result.clone()),
        };
        self.credential_store.save(stored).await?;

        Ok(token_result)
    }

    /// get access token, if expired, refresh it automatically
    pub async fn get_access_token(&self) -> Result<String, AuthError> {
        // Load credentials from store
        let stored = self.credential_store.load().await?;
        let credentials = stored.and_then(|s| s.token_response);

        if let Some(creds) = credentials.as_ref() {
            // check token expiry if we have a refresh token or an expiry time
            if creds.refresh_token().is_some() || creds.expires_in().is_some() {
                let expires_in = creds.expires_in().unwrap_or(Duration::from_secs(0));
                if expires_in <= Duration::from_secs(0) {
                    tracing::info!("Access token expired, refreshing.");

                    let new_creds = self.refresh_token().await?;
                    tracing::info!("Refreshed access token.");
                    return Ok(new_creds.access_token().secret().to_string());
                }
            }

            Ok(creds.access_token().secret().to_string())
        } else {
            Err(AuthError::AuthorizationRequired)
        }
    }

    /// refresh access token
    pub async fn refresh_token(
        &self,
    ) -> Result<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>, AuthError> {
        let oauth_client = self
            .oauth_client
            .as_ref()
            .ok_or_else(|| AuthError::InternalError("OAuth client not configured".to_string()))?;

        let stored = self.credential_store.load().await?;
        let current_credentials = stored
            .and_then(|s| s.token_response)
            .ok_or_else(|| AuthError::AuthorizationRequired)?;

        let refresh_token = current_credentials.refresh_token().ok_or_else(|| {
            AuthError::TokenRefreshFailed("No refresh token available".to_string())
        })?;
        debug!("refresh token: {:?}", refresh_token);
        let http_client = self.http_client.clone();

        let token_result = oauth_client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.secret().to_string()))
            .request_async(&move |request| {
                let http_client = http_client.clone();
                async move { http_client.execute(request).await }
            })
            .await
            .map_err(|e| AuthError::TokenRefreshFailed(e.to_string()))?;

        let client_id = oauth_client.client_id().to_string();
        let stored = StoredCredentials {
            client_id,
            token_response: Some(token_result.clone()),
        };
        self.credential_store.save(stored).await?;

        Ok(token_result)
    }

    /// prepare request, add authorization header
    pub async fn prepare_request(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, AuthError> {
        let token = self.get_access_token().await?;
        Ok(request.header(AUTHORIZATION, format!("Bearer {}", token)))
    }

    /// handle response, check if need to re-authorize
    pub async fn handle_response(
        &self,
        response: reqwest::Response,
    ) -> Result<reqwest::Response, AuthError> {
        if response.status() == StatusCode::UNAUTHORIZED {
            // 401 Unauthorized, need to re-authorize
            Err(AuthError::AuthorizationRequired)
        } else {
            Ok(response)
        }
    }

    /// Generate discovery endpoint URLs following the priority order in spec-2025-11-25 4.3 "Authorization Server Metadata Discovery".
    fn generate_discovery_urls(base_url: &Url) -> Vec<Url> {
        let mut candidates = Vec::new();
        let path = base_url.path();
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        let mut push_candidate = |discovery_path: String| {
            let mut discovery_url = base_url.clone();
            discovery_url.set_query(None);
            discovery_url.set_fragment(None);
            discovery_url.set_path(&discovery_path);
            candidates.push(discovery_url);
        };
        if trimmed.is_empty() {
            // No path components: try OAuth first, then OpenID Connect
            push_candidate("/.well-known/oauth-authorization-server".to_string());
            push_candidate("/.well-known/openid-configuration".to_string());
        } else {
            // Path components present: follow spec priority order
            // 1. OAuth 2.0 with path insertion
            push_candidate(format!("/.well-known/oauth-authorization-server/{trimmed}"));
            // 2. OpenID Connect with path insertion
            push_candidate(format!("/.well-known/openid-configuration/{trimmed}"));
            // 3. OpenID Connect with path appending
            push_candidate(format!("/{trimmed}/.well-known/openid-configuration"));
            // 4. Canonical OAuth fallback (without path suffix)
            push_candidate("/.well-known/oauth-authorization-server".to_string());
        }

        candidates
    }

    async fn try_discover_oauth_server(
        &self,
        base_url: &Url,
    ) -> Result<Option<AuthorizationMetadata>, AuthError> {
        for discovery_url in Self::generate_discovery_urls(base_url) {
            if let Some(metadata) = self.fetch_authorization_metadata(&discovery_url).await? {
                return Ok(Some(metadata));
            }
        }
        Ok(None)
    }

    async fn fetch_authorization_metadata(
        &self,
        discovery_url: &Url,
    ) -> Result<Option<AuthorizationMetadata>, AuthError> {
        debug!("discovery url: {:?}", discovery_url);
        let response = match self
            .send_request(self.build_request(
                reqwest::Method::GET,
                discovery_url,
                Vec::new(),
                [("MCP-Protocol-Version", "2024-11-05")].as_slice(),
            )?)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("discovery request failed: {}", e);
                return Ok(None);
            }
        };

        if response.status() != StatusCode::OK {
            debug!("discovery returned non-200: {}", response.status());
            return Ok(None);
        }

        match serde_json::from_slice::<AuthorizationMetadata>(response.body()) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(err) => {
                debug!("Failed to parse metadata for {}: {}", discovery_url, err);
                Ok(None) // malformed JSON ⇒ try next candidate
            }
        }
    }

    async fn discover_oauth_server_via_resource_metadata(
        &self,
    ) -> Result<Option<AuthorizationMetadata>, AuthError> {
        let Some(resource_metadata_url) = self.discover_resource_metadata_url().await? else {
            return Ok(None);
        };

        let Some(resource_metadata) = self
            .fetch_resource_metadata_from_url(&resource_metadata_url)
            .await?
        else {
            return Ok(None);
        };

        let mut candidates = Vec::new();

        if let Some(single) = resource_metadata.authorization_server {
            candidates.push(single);
        }
        if let Some(list) = resource_metadata.authorization_servers {
            candidates.extend(list);
        }

        for candidate in candidates {
            let candidate = candidate.trim();
            if candidate.is_empty() {
                continue;
            }

            let candidate_url = match Url::parse(candidate) {
                Ok(url) => url,
                Err(_) => match resource_metadata_url.join(candidate) {
                    Ok(url) => url,
                    Err(e) => {
                        debug!("Failed to resolve authorization server URL `{candidate}`: {e}");
                        continue;
                    }
                },
            };

            if candidate_url.path().contains("/.well-known/") {
                if let Some(metadata) = self.fetch_authorization_metadata(&candidate_url).await? {
                    return Ok(Some(metadata));
                }
                continue;
            }

            if let Some(metadata) = self.try_discover_oauth_server(&candidate_url).await? {
                return Ok(Some(metadata));
            }
        }

        Ok(None)
    }

    async fn discover_resource_metadata_url(&self) -> Result<Option<Url>, AuthError> {
        if let Ok(Some(resource_metadata_url)) =
            self.fetch_resource_metadata_url(&self.base_url).await
        {
            return Ok(Some(resource_metadata_url));
        }

        // If the primary URL doesn't use WWW-Authenticate, try oauth-protected-resource discovery.
        // https://www.rfc-editor.org/rfc/rfc9728.html#name-obtaining-protected-resourc
        for candidate_path in
            Self::well_known_paths(self.base_url.path(), "oauth-protected-resource")
        {
            let mut discovery_url = self.base_url.clone();
            discovery_url.set_query(None);
            discovery_url.set_fragment(None);
            discovery_url.set_path(&candidate_path);
            if let Ok(Some(resource_metadata_url)) =
                self.fetch_resource_metadata_url(&discovery_url).await
            {
                return Ok(Some(resource_metadata_url));
            }
        }

        Ok(None)
    }

    /// Extract the resource metadata url from the WWW-Authenticate header value.
    /// https://www.rfc-editor.org/rfc/rfc9728.html#name-use-of-www-authenticate-for
    async fn fetch_resource_metadata_url(&self, url: &Url) -> Result<Option<Url>, AuthError> {
        let response = match self
            .send_request(self.build_request(
                reqwest::Method::GET,
                url,
                Vec::new(),
                [("MCP-Protocol-Version", "2024-11-05")].as_slice(),
            )?)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("resource metadata probe failed: {}", e);
                return Ok(None);
            }
        };

        if response.status() == StatusCode::OK {
            return Ok(Some(url.clone()));
        } else if response.status() != StatusCode::UNAUTHORIZED {
            debug!(
                "resource metadata probe returned unexpected status: {}",
                response.status()
            );
            return Ok(None);
        }

        let mut parsed_url = None;
        for value in response.headers().get_all(WWW_AUTHENTICATE).iter() {
            let Ok(value_str) = value.to_str() else {
                continue;
            };
            if let Some(url) =
                Self::extract_resource_metadata_url_from_header(value_str, &self.base_url)
            {
                parsed_url = Some(url);
                break;
            }
        }

        Ok(parsed_url)
    }

    async fn fetch_resource_metadata_from_url(
        &self,
        resource_metadata_url: &Url,
    ) -> Result<Option<ResourceServerMetadata>, AuthError> {
        debug!(
            "resource metadata discovery url: {:?}",
            resource_metadata_url
        );
        let response = match self
            .send_request(self.build_request(
                reqwest::Method::GET,
                resource_metadata_url,
                Vec::new(),
                [("MCP-Protocol-Version", "2024-11-05")].as_slice(),
            )?)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("resource metadata request failed: {}", e);
                return Ok(None);
            }
        };

        if response.status() != StatusCode::OK {
            debug!(
                "resource metadata request returned non-200: {}",
                response.status()
            );
            return Ok(None);
        }

        let metadata =
            serde_json::from_slice::<ResourceServerMetadata>(response.body()).map_err(|e| {
                AuthError::MetadataError(format!("Failed to parse resource metadata: {}", e))
            })?;
        Ok(Some(metadata))
    }

    /// Extracts a url following `resource_metadata=` in a header value
    fn extract_resource_metadata_url_from_header(header: &str, base_url: &Url) -> Option<Url> {
        let header_lowercase = header.to_ascii_lowercase();
        let fragment_key = "resource_metadata=";
        let mut search_offset = 0;

        while let Some(pos) = header_lowercase[search_offset..].find(fragment_key) {
            let global_pos = search_offset + pos + fragment_key.len();
            let value_slice = &header[global_pos..];
            if let Some((value, consumed)) = Self::parse_next_header_value(value_slice) {
                if let Ok(url) = Url::parse(&value) {
                    return Some(url);
                }
                if let Ok(url) = base_url.join(&value) {
                    return Some(url);
                }
                debug!("failed to parse resource metadata value `{value}` as URL");
                search_offset = global_pos + consumed;
                continue;
            } else {
                break;
            }
        }

        None
    }

    /// Parses an authentication parameter value from a `WWW-Authenticate` header fragment.
    /// The header fragment should start with the header value after the `=` character and then
    /// reads until the value ends.
    ///
    /// Returns the extracted value together with the number of bytes consumed from the provided
    /// fragment. Quoted values support escaped characters (e.g. `\"`). The parser skips leading
    /// whitespace before reading either a quoted or token value. If no well-formed value is found,
    /// `None` is returned.
    fn parse_next_header_value(header_fragment: &str) -> Option<(String, usize)> {
        let trimmed = header_fragment.trim_start();
        let leading_ws = header_fragment.len() - trimmed.len();

        if let Some(stripped) = trimmed.strip_prefix('"') {
            let mut escaped = false;
            let mut result = String::new();
            #[allow(clippy::manual_strip)]
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
}

/// oauth2 authorization session, for guiding user to complete the authorization process
pub struct AuthorizationSession {
    pub auth_manager: AuthorizationManager,
    pub auth_url: String,
    pub redirect_uri: String,
}

impl AuthorizationSession {
    /// create new authorization session
    pub async fn new(
        mut auth_manager: AuthorizationManager,
        scopes: &[&str],
        redirect_uri: &str,
        client_name: Option<&str>,
        client_metadata_url: Option<&str>,
    ) -> Result<Self, AuthError> {
        let metadata = auth_manager.metadata.as_ref();
        let supports_url_based_client_id = metadata
            .and_then(|m| {
                m.additional_fields
                    .get("client_id_metadata_document_supported")
            })
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let config = if supports_url_based_client_id {
            if let Some(client_metadata_url) = client_metadata_url {
                if !is_https_url(client_metadata_url) {
                    return Err(AuthError::RegistrationFailed(format!(
                        "client_metadata_url must be a valid HTTPS URL with a non-root pathname, got: {}",
                        client_metadata_url
                    )));
                }
                // SEP-991: URL-based Client IDs - use URL as client_id directly
                OAuthClientConfig {
                    client_id: client_metadata_url.to_string(),
                    client_secret: None,
                    scopes: scopes.iter().map(|s| s.to_string()).collect(),
                    redirect_uri: redirect_uri.to_string(),
                }
            } else {
                // Fallback to dynamic registration
                auth_manager
                    .register_client(client_name.unwrap_or("MCP Client"), redirect_uri)
                    .await
                    .map_err(|e| {
                        AuthError::RegistrationFailed(format!("Dynamic registration failed: {}", e))
                    })?
            }
        } else {
            // Fallback to dynamic registration
            match auth_manager
                .register_client(client_name.unwrap_or("MCP Client"), redirect_uri)
                .await
            {
                Ok(config) => config,
                Err(e) => {
                    return Err(AuthError::RegistrationFailed(format!(
                        "Dynamic registration failed: {}",
                        e
                    )));
                }
            }
        };

        // reset client config
        auth_manager.configure_client(config)?;
        let auth_url = auth_manager.get_authorization_url(scopes).await?;

        Ok(Self {
            auth_manager,
            auth_url,
            redirect_uri: redirect_uri.to_string(),
        })
    }

    /// get client_id and credentials
    pub async fn get_credentials(&self) -> Result<Credentials, AuthError> {
        self.auth_manager.get_credentials().await
    }

    /// get authorization url
    pub fn get_authorization_url(&self) -> &str {
        &self.auth_url
    }

    /// handle authorization code callback
    pub async fn handle_callback(
        &self,
        code: &str,
        csrf_token: &str,
    ) -> Result<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>, AuthError> {
        self.auth_manager
            .exchange_code_for_token(code, csrf_token)
            .await
    }
}

/// http client extension, automatically add authorization header
pub struct AuthorizedHttpClient {
    auth_manager: Arc<AuthorizationManager>,
    inner_client: HttpClient,
}

impl AuthorizedHttpClient {
    /// create new authorized http client
    pub fn new(auth_manager: Arc<AuthorizationManager>, client: Option<HttpClient>) -> Self {
        let inner_client = client.unwrap_or_default();
        Self {
            auth_manager,
            inner_client,
        }
    }

    /// send authorized request
    pub async fn request<U: IntoUrl>(
        &self,
        method: reqwest::Method,
        url: U,
    ) -> Result<reqwest::RequestBuilder, AuthError> {
        let request = self.inner_client.request(method, url);
        self.auth_manager.prepare_request(request).await
    }

    /// send get request
    pub async fn get<U: IntoUrl>(&self, url: U) -> Result<reqwest::Response, AuthError> {
        let request = self.request(reqwest::Method::GET, url).await?;
        let response = request.send().await?;
        self.auth_manager.handle_response(response).await
    }

    /// send post request
    pub async fn post<U: IntoUrl>(&self, url: U) -> Result<reqwest::RequestBuilder, AuthError> {
        self.request(reqwest::Method::POST, url).await
    }
}

/// OAuth state machine
/// Use the OAuthState to manage the OAuth client is more recommend
/// But also you can use the AuthorizationManager,AuthorizationSession,AuthorizedHttpClient directly
pub enum OAuthState {
    /// the AuthorizationManager
    Unauthorized(AuthorizationManager),
    /// the AuthorizationSession
    Session(AuthorizationSession),
    /// the authd AuthorizationManager
    Authorized(AuthorizationManager),
    /// the authd http client
    AuthorizedHttpClient(AuthorizedHttpClient),
}

impl OAuthState {
    /// Create new OAuth state machine
    pub async fn new<U: IntoUrl>(
        base_url: U,
        client: Option<HttpClient>,
    ) -> Result<Self, AuthError> {
        let mut manager = AuthorizationManager::new(base_url).await?;
        if let Some(client) = client {
            manager.with_client(client)?;
        }

        Ok(OAuthState::Unauthorized(manager))
    }

    /// Get client_id and OAuth credentials
    pub async fn get_credentials(&self) -> Result<Credentials, AuthError> {
        // return client_id and credentials
        match self {
            OAuthState::Unauthorized(manager) | OAuthState::Authorized(manager) => {
                manager.get_credentials().await
            }
            OAuthState::Session(session) => session.get_credentials().await,
            OAuthState::AuthorizedHttpClient(client) => client.auth_manager.get_credentials().await,
        }
    }

    /// Manually set credentials and move into authorized state
    /// Useful if you're caching credentials externally and wish to reuse them
    pub async fn set_credentials(
        &mut self,
        client_id: &str,
        credentials: OAuthTokenResponse,
    ) -> Result<(), AuthError> {
        if let OAuthState::Unauthorized(manager) = self {
            let mut manager = std::mem::replace(
                manager,
                AuthorizationManager::new(DEFAULT_EXCHANGE_URL).await?,
            );

            let stored = StoredCredentials {
                client_id: client_id.to_string(),
                token_response: Some(credentials),
            };
            manager.credential_store.save(stored).await?;

            let metadata = manager.discover_metadata().await?;
            manager.metadata = Some(metadata);

            manager.configure_client_id(client_id)?;

            *self = OAuthState::Authorized(manager);
            Ok(())
        } else {
            Err(AuthError::InternalError(
                "Cannot set credentials in this state".to_string(),
            ))
        }
    }

    /// start authorization
    pub async fn start_authorization(
        &mut self,
        scopes: &[&str],
        redirect_uri: &str,
        client_name: Option<&str>,
    ) -> Result<(), AuthError> {
        self.start_authorization_with_metadata_url(scopes, redirect_uri, client_name, None)
            .await
    }

    /// start authorization with optional client metadata URL (SEP-991)
    pub async fn start_authorization_with_metadata_url(
        &mut self,
        scopes: &[&str],
        redirect_uri: &str,
        client_name: Option<&str>,
        client_metadata_url: Option<&str>,
    ) -> Result<(), AuthError> {
        if let OAuthState::Unauthorized(mut manager) = std::mem::replace(
            self,
            OAuthState::Unauthorized(AuthorizationManager::new(DEFAULT_EXCHANGE_URL).await?),
        ) {
            debug!("start discovery");
            let metadata = manager.discover_metadata().await?;
            manager.metadata = Some(metadata);
            debug!("start session");
            let session = AuthorizationSession::new(
                manager,
                scopes,
                redirect_uri,
                client_name,
                client_metadata_url,
            )
            .await?;
            *self = OAuthState::Session(session);
            Ok(())
        } else {
            Err(AuthError::InternalError(
                "Already in session state".to_string(),
            ))
        }
    }

    /// complete authorization
    pub async fn complete_authorization(&mut self) -> Result<(), AuthError> {
        if let OAuthState::Session(session) = std::mem::replace(
            self,
            OAuthState::Unauthorized(AuthorizationManager::new(DEFAULT_EXCHANGE_URL).await?),
        ) {
            *self = OAuthState::Authorized(session.auth_manager);
            Ok(())
        } else {
            Err(AuthError::InternalError("Not in session state".to_string()))
        }
    }
    /// covert to authorized http client
    pub async fn to_authorized_http_client(&mut self) -> Result<(), AuthError> {
        if let OAuthState::Authorized(manager) = std::mem::replace(
            self,
            OAuthState::Authorized(AuthorizationManager::new(DEFAULT_EXCHANGE_URL).await?),
        ) {
            *self = OAuthState::AuthorizedHttpClient(AuthorizedHttpClient::new(
                Arc::new(manager),
                None,
            ));
            Ok(())
        } else {
            Err(AuthError::InternalError(
                "Not in authorized state".to_string(),
            ))
        }
    }
    /// get current authorization url
    pub async fn get_authorization_url(&self) -> Result<String, AuthError> {
        match self {
            OAuthState::Session(session) => Ok(session.get_authorization_url().to_string()),
            OAuthState::Unauthorized(_) => {
                Err(AuthError::InternalError("Not in session state".to_string()))
            }
            OAuthState::Authorized(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
            OAuthState::AuthorizedHttpClient(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
        }
    }

    /// handle authorization callback
    pub async fn handle_callback(&mut self, code: &str, csrf_token: &str) -> Result<(), AuthError> {
        match self {
            OAuthState::Session(session) => {
                session.handle_callback(code, csrf_token).await?;
                self.complete_authorization().await
            }
            OAuthState::Unauthorized(_) => {
                Err(AuthError::InternalError("Not in session state".to_string()))
            }
            OAuthState::Authorized(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
            OAuthState::AuthorizedHttpClient(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
        }
    }

    /// get access token
    pub async fn get_access_token(&self) -> Result<String, AuthError> {
        match self {
            OAuthState::Unauthorized(manager) => manager.get_access_token().await,
            OAuthState::Session(_) => {
                Err(AuthError::InternalError("Not in manager state".to_string()))
            }
            OAuthState::Authorized(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
            OAuthState::AuthorizedHttpClient(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
        }
    }

    /// refresh access token
    pub async fn refresh_token(&self) -> Result<(), AuthError> {
        match self {
            OAuthState::Unauthorized(_) => {
                Err(AuthError::InternalError("Not in manager state".to_string()))
            }
            OAuthState::Session(_) => {
                Err(AuthError::InternalError("Not in manager state".to_string()))
            }
            OAuthState::Authorized(manager) => {
                manager.refresh_token().await?;
                Ok(())
            }
            OAuthState::AuthorizedHttpClient(_) => {
                Err(AuthError::InternalError("Already authorized".to_string()))
            }
        }
    }

    pub fn into_authorization_manager(self) -> Option<AuthorizationManager> {
        match self {
            OAuthState::Authorized(manager) => Some(manager),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use oauth2::{CsrfToken, PkceCodeVerifier};
    use url::Url;

    use super::{
        AuthError, AuthorizationManager, InMemoryStateStore, StateStore, StoredAuthorizationState,
        is_https_url,
    };

    // SEP-991: URL-based Client IDs
    // Tests adapted from the TypeScript SDK's isHttpsUrl test suite
    #[test]
    fn test_is_https_url_scenarios() {
        // Returns true for valid https url with path
        assert!(is_https_url("https://example.com/client-metadata.json"));
        // Returns true for https url with query params
        assert!(is_https_url("https://example.com/metadata?version=1"));
        // Returns false for https url without path
        assert!(!is_https_url("https://example.com"));
        assert!(!is_https_url("https://example.com/"));
        assert!(!is_https_url("https://"));
        // Returns false for http url
        assert!(!is_https_url("http://example.com/metadata"));
        // Returns false for non-url strings
        assert!(!is_https_url("not a url"));
        // Returns false for empty string
        assert!(!is_https_url(""));
        // Returns false for javascript scheme
        assert!(!is_https_url("javascript:alert(1)"));
        // Returns false for data scheme
        assert!(!is_https_url("data:text/html,<script>alert(1)</script>"));
    }

    #[test]
    fn parses_resource_metadata_parameter() {
        let header = r#"Bearer error="invalid_request", error_description="missing token", resource_metadata="https://example.com/.well-known/oauth-protected-resource/api""#;
        let base = Url::parse("https://example.com/api").unwrap();
        let parsed = AuthorizationManager::extract_resource_metadata_url_from_header(header, &base);
        assert_eq!(
            parsed.unwrap().as_str(),
            "https://example.com/.well-known/oauth-protected-resource/api"
        );
    }

    #[test]
    fn parses_relative_resource_metadata_parameter() {
        let header = r#"Bearer error="invalid_request", resource_metadata="/.well-known/oauth-protected-resource/api""#;
        let base = Url::parse("https://example.com/api").unwrap();
        let parsed = AuthorizationManager::extract_resource_metadata_url_from_header(header, &base);
        assert_eq!(
            parsed.unwrap().as_str(),
            "https://example.com/.well-known/oauth-protected-resource/api"
        );
    }

    #[test]
    fn parse_auth_param_value_handles_quoted_string() {
        let fragment = r#""example", realm="foo""#;
        let parsed = AuthorizationManager::parse_next_header_value(fragment).unwrap();
        assert_eq!(parsed.0, "example");
        assert_eq!(parsed.1, 9);
    }

    #[test]
    fn parse_auth_param_value_handles_escaped_quotes_and_whitespace() {
        let fragment = r#"   "a\"b\\c" ,next=value"#;
        let parsed = AuthorizationManager::parse_next_header_value(fragment).unwrap();
        assert_eq!(parsed.0, r#"a"b\c"#);
        assert_eq!(parsed.1, 12);
    }

    #[test]
    fn parse_auth_param_value_handles_token_values() {
        let fragment = "  token,next";
        let parsed = AuthorizationManager::parse_next_header_value(fragment).unwrap();
        assert_eq!(parsed.0, "token");
        assert_eq!(parsed.1, 7);
    }

    #[test]
    fn parse_auth_param_value_handles_semicolon_separated_tokens() {
        let fragment = r#"  https://example.com/meta; error="invalid_token""#;
        let parsed = AuthorizationManager::parse_next_header_value(fragment).unwrap();
        assert_eq!(parsed.0, "https://example.com/meta");
        assert_eq!(&fragment[..parsed.1], "  https://example.com/meta");
    }

    #[test]
    fn parse_auth_param_value_handles_semicolon_after_quoted_value() {
        let fragment = r#"  "https://example.com/meta"; error="invalid_token""#;
        let parsed = AuthorizationManager::parse_next_header_value(fragment).unwrap();
        assert_eq!(parsed.0, "https://example.com/meta");
        assert_eq!(&fragment[..parsed.1], r#"  "https://example.com/meta""#);
    }

    #[test]
    fn parse_auth_param_value_returns_none_for_unterminated_quotes() {
        let fragment = r#""unterminated,value"#;
        assert!(AuthorizationManager::parse_next_header_value(fragment).is_none());
    }

    #[test]
    fn well_known_paths_root() {
        let paths = AuthorizationManager::well_known_paths("/", "oauth-authorization-server");
        assert_eq!(
            paths,
            vec!["/.well-known/oauth-authorization-server".to_string()]
        );
    }

    #[test]
    fn well_known_paths_with_suffix() {
        let paths = AuthorizationManager::well_known_paths("/mcp", "oauth-authorization-server");
        assert_eq!(
            paths,
            vec![
                "/.well-known/oauth-authorization-server/mcp".to_string(),
                "/mcp/.well-known/oauth-authorization-server".to_string(),
                "/.well-known/oauth-authorization-server".to_string(),
            ]
        );
    }

    #[test]
    fn well_known_paths_trailing_slash() {
        let paths =
            AuthorizationManager::well_known_paths("/v1/mcp/", "oauth-authorization-server");
        assert_eq!(
            paths,
            vec![
                "/.well-known/oauth-authorization-server/v1/mcp".to_string(),
                "/v1/mcp/.well-known/oauth-authorization-server".to_string(),
                "/.well-known/oauth-authorization-server".to_string(),
            ]
        );
    }

    #[test]
    fn generate_discovery_urls() {
        // Test root URL (no path components): OAuth first, then OpenID Connect
        let base_url = Url::parse("https://auth.example.com").unwrap();
        let urls = AuthorizationManager::generate_discovery_urls(&base_url);
        assert_eq!(urls.len(), 2);
        assert_eq!(
            urls[0].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );
        assert_eq!(
            urls[1].as_str(),
            "https://auth.example.com/.well-known/openid-configuration"
        );

        // Test URL with single path segment: follow spec priority order
        let base_url = Url::parse("https://auth.example.com/tenant1").unwrap();
        let urls = AuthorizationManager::generate_discovery_urls(&base_url);
        assert_eq!(urls.len(), 4);
        assert_eq!(
            urls[0].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server/tenant1"
        );
        assert_eq!(
            urls[1].as_str(),
            "https://auth.example.com/.well-known/openid-configuration/tenant1"
        );
        assert_eq!(
            urls[2].as_str(),
            "https://auth.example.com/tenant1/.well-known/openid-configuration"
        );
        assert_eq!(
            urls[3].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );

        // Test URL with path and trailing slash
        let base_url = Url::parse("https://auth.example.com/v1/mcp/").unwrap();
        let urls = AuthorizationManager::generate_discovery_urls(&base_url);
        assert_eq!(urls.len(), 4);
        assert_eq!(
            urls[0].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server/v1/mcp"
        );
        assert_eq!(
            urls[1].as_str(),
            "https://auth.example.com/.well-known/openid-configuration/v1/mcp"
        );
        assert_eq!(
            urls[2].as_str(),
            "https://auth.example.com/v1/mcp/.well-known/openid-configuration"
        );
        assert_eq!(
            urls[3].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );

        // Test URL with multiple path segments
        let base_url = Url::parse("https://auth.example.com/tenant1/subtenant").unwrap();
        let urls = AuthorizationManager::generate_discovery_urls(&base_url);
        assert_eq!(urls.len(), 4);
        assert_eq!(
            urls[0].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server/tenant1/subtenant"
        );
        assert_eq!(
            urls[1].as_str(),
            "https://auth.example.com/.well-known/openid-configuration/tenant1/subtenant"
        );
        assert_eq!(
            urls[2].as_str(),
            "https://auth.example.com/tenant1/subtenant/.well-known/openid-configuration"
        );
        assert_eq!(
            urls[3].as_str(),
            "https://auth.example.com/.well-known/oauth-authorization-server"
        );
    }

    // StateStore and StoredAuthorizationState tests

    #[tokio::test]
    async fn test_in_memory_state_store_save_and_load() {
        let store = InMemoryStateStore::new();
        let pkce = PkceCodeVerifier::new("test-verifier".to_string());
        let csrf = CsrfToken::new("test-csrf".to_string());
        let state = StoredAuthorizationState::new(&pkce, &csrf);

        // Save state
        store.save("test-csrf", state).await.unwrap();

        // Load state
        let loaded = store.load("test-csrf").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.csrf_token, "test-csrf");
        assert_eq!(loaded.pkce_verifier, "test-verifier");
    }

    #[tokio::test]
    async fn test_in_memory_state_store_load_nonexistent() {
        let store = InMemoryStateStore::new();
        let result = store.load("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_state_store_delete() {
        let store = InMemoryStateStore::new();
        let pkce = PkceCodeVerifier::new("verifier".to_string());
        let csrf = CsrfToken::new("csrf".to_string());
        let state = StoredAuthorizationState::new(&pkce, &csrf);

        store.save("csrf", state).await.unwrap();
        store.delete("csrf").await.unwrap();

        let result = store.load("csrf").await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_stored_authorization_state_serialization() {
        let pkce = PkceCodeVerifier::new("my-verifier".to_string());
        let csrf = CsrfToken::new("my-csrf".to_string());
        let state = StoredAuthorizationState::new(&pkce, &csrf);

        // Serialize to JSON
        let json = serde_json::to_string(&state).unwrap();

        // Deserialize back
        let deserialized: StoredAuthorizationState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.pkce_verifier, "my-verifier");
        assert_eq!(deserialized.csrf_token, "my-csrf");
    }

    #[test]
    fn test_stored_authorization_state_into_pkce_verifier() {
        let pkce = PkceCodeVerifier::new("original-verifier".to_string());
        let csrf = CsrfToken::new("csrf-token".to_string());
        let state = StoredAuthorizationState::new(&pkce, &csrf);

        let recovered = state.into_pkce_verifier();
        assert_eq!(recovered.secret(), "original-verifier");
    }

    #[test]
    fn test_stored_authorization_state_created_at() {
        let pkce = PkceCodeVerifier::new("verifier".to_string());
        let csrf = CsrfToken::new("csrf".to_string());
        let state = StoredAuthorizationState::new(&pkce, &csrf);

        // created_at should be a reasonable timestamp (after year 2020)
        assert!(state.created_at > 1577836800); // Jan 1, 2020
    }

    #[tokio::test]
    async fn test_in_memory_state_store_overwrite() {
        let store = InMemoryStateStore::new();
        let csrf_key = "same-csrf";

        // Save first state
        let pkce1 = PkceCodeVerifier::new("verifier-1".to_string());
        let csrf1 = CsrfToken::new(csrf_key.to_string());
        let state1 = StoredAuthorizationState::new(&pkce1, &csrf1);
        store.save(csrf_key, state1).await.unwrap();

        // Overwrite with second state
        let pkce2 = PkceCodeVerifier::new("verifier-2".to_string());
        let csrf2 = CsrfToken::new(csrf_key.to_string());
        let state2 = StoredAuthorizationState::new(&pkce2, &csrf2);
        store.save(csrf_key, state2).await.unwrap();

        // Should get the second state
        let loaded = store.load(csrf_key).await.unwrap().unwrap();
        assert_eq!(loaded.pkce_verifier, "verifier-2");
    }

    #[tokio::test]
    async fn test_in_memory_state_store_concurrent_access() {
        let store = Arc::new(InMemoryStateStore::new());
        let mut handles = vec![];

        // Spawn 10 concurrent tasks that each save and load their own state
        for i in 0..10 {
            let store = Arc::clone(&store);
            let handle = tokio::spawn(async move {
                let csrf_key = format!("csrf-{}", i);
                let verifier = format!("verifier-{}", i);

                let pkce = PkceCodeVerifier::new(verifier.clone());
                let csrf = CsrfToken::new(csrf_key.clone());
                let state = StoredAuthorizationState::new(&pkce, &csrf);

                store.save(&csrf_key, state).await.unwrap();
                let loaded = store.load(&csrf_key).await.unwrap().unwrap();
                assert_eq!(loaded.pkce_verifier, verifier);

                store.delete(&csrf_key).await.unwrap();
                let deleted = store.load(&csrf_key).await.unwrap();
                assert!(deleted.is_none());
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[test]
    fn test_discovery_urls_with_path_suffix() {
        // When the base URL has a path suffix (e.g., /mcp), the discovery should
        // eventually fall back to checking /.well-known/oauth-authorization-server
        // at the root, not just /.well-known/oauth-authorization-server/{path}.
        let base_url = Url::parse("https://mcp.example.com/mcp").unwrap();
        let urls = AuthorizationManager::generate_discovery_urls(&base_url);

        let canonical_oauth_fallback =
            "https://mcp.example.com/.well-known/oauth-authorization-server";

        assert!(
            urls.iter().any(|u| u.as_str() == canonical_oauth_fallback),
            "Expected discovery URLs to include canonical OAuth fallback '{}', but got: {:?}",
            canonical_oauth_fallback,
            urls.iter().map(|u| u.as_str()).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_custom_state_store_with_authorization_manager() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Custom state store that tracks calls
        #[derive(Debug, Default)]
        struct TrackingStateStore {
            inner: InMemoryStateStore,
            save_count: AtomicUsize,
            load_count: AtomicUsize,
            delete_count: AtomicUsize,
        }

        #[async_trait::async_trait]
        impl StateStore for TrackingStateStore {
            async fn save(
                &self,
                csrf_token: &str,
                state: StoredAuthorizationState,
            ) -> Result<(), AuthError> {
                self.save_count.fetch_add(1, Ordering::SeqCst);
                self.inner.save(csrf_token, state).await
            }

            async fn load(
                &self,
                csrf_token: &str,
            ) -> Result<Option<StoredAuthorizationState>, AuthError> {
                self.load_count.fetch_add(1, Ordering::SeqCst);
                self.inner.load(csrf_token).await
            }

            async fn delete(&self, csrf_token: &str) -> Result<(), AuthError> {
                self.delete_count.fetch_add(1, Ordering::SeqCst);
                self.inner.delete(csrf_token).await
            }
        }

        // Verify custom store works standalone
        let store = TrackingStateStore::default();
        let pkce = PkceCodeVerifier::new("test-verifier".to_string());
        let csrf = CsrfToken::new("test-csrf".to_string());
        let state = StoredAuthorizationState::new(&pkce, &csrf);

        store.save("test-csrf", state).await.unwrap();
        assert_eq!(store.save_count.load(Ordering::SeqCst), 1);

        let _ = store.load("test-csrf").await.unwrap();
        assert_eq!(store.load_count.load(Ordering::SeqCst), 1);

        store.delete("test-csrf").await.unwrap();
        assert_eq!(store.delete_count.load(Ordering::SeqCst), 1);

        // Verify custom store can be set on AuthorizationManager
        let mut manager = AuthorizationManager::new("http://localhost").await.unwrap();
        manager.set_state_store(TrackingStateStore::default());
    }
}
