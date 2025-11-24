use std::{collections::HashMap, sync::Arc, time::Duration};

use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields,
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
use tracing::{debug, error, warn};

const DEFAULT_EXCHANGE_URL: &str = "http://localhost";

/// sse client with oauth2 authorization
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
    /// create new authorized sse client
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizationMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    pub issuer: Option<String>,
    pub jwks_uri: Option<String>,
    pub scopes_supported: Option<Vec<String>>,
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

/// oauth2 auth manager
pub struct AuthorizationManager {
    http_client: HttpClient,
    metadata: Option<AuthorizationMetadata>,
    oauth_client: Option<OAuthClient>,
    credentials: RwLock<Option<OAuthTokenResponse>>,
    state: RwLock<Option<AuthorizationState>>,
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

#[derive(Debug)]
struct AuthorizationState {
    pkce_verifier: PkceCodeVerifier,
    csrf_token: CsrfToken,
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
            http_client,
            metadata: None,
            oauth_client: None,
            credentials: RwLock::new(None),
            state: RwLock::new(None),
            base_url,
        };

        Ok(manager)
    }

    pub fn with_client(&mut self, http_client: HttpClient) -> Result<(), AuthError> {
        self.http_client = http_client;
        Ok(())
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
        let credentials = self.credentials.read().await;
        let client_id = self
            .oauth_client
            .as_ref()
            .ok_or_else(|| AuthError::InternalError("OAuth client not configured".to_string()))?
            .client_id();
        Ok((client_id.to_string(), credentials.clone()))
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

        // prepare registration request
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

        let response = match self
            .http_client
            .post(registration_url)
            .json(&registration_request)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return Err(AuthError::RegistrationFailed(format!(
                    "HTTP request error: {}",
                    e
                )));
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = match response.text().await {
                Ok(text) => text,
                Err(_) => "cannot get error details".to_string(),
            };

            return Err(AuthError::RegistrationFailed(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        debug!("registration response: {:?}", response);
        let reg_response = match response.json::<ClientRegistrationResponse>().await {
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

        // store pkce verifier for later use
        *self.state.write().await = Some(AuthorizationState {
            pkce_verifier,
            csrf_token,
        });

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

        let AuthorizationState {
            pkce_verifier,
            csrf_token: expected_csrf_token,
        } =
            self.state.write().await.take().ok_or_else(|| {
                AuthError::InternalError("Authorization state not found".to_string())
            })?;

        if csrf_token != expected_csrf_token.secret() {
            return Err(AuthError::InternalError("CSRF token mismatch".to_string()));
        }

        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        debug!("client_id: {:?}", oauth_client.client_id());

        // exchange token
        let token_result = match oauth_client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
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
        // store credentials
        *self.credentials.write().await = Some(token_result.clone());

        Ok(token_result)
    }

    /// get access token, if expired, refresh it automatically
    pub async fn get_access_token(&self) -> Result<String, AuthError> {
        let credentials = self.credentials.read().await;

        if let Some(creds) = credentials.as_ref() {
            // check if the token is expire
            let expires_in = creds.expires_in().unwrap_or(Duration::from_secs(0));
            if expires_in <= Duration::from_secs(0) {
                tracing::info!("Access token expired, refreshing.");
                // token expired, try to refresh , release the lock
                drop(credentials);

                let new_creds = self.refresh_token().await?;
                tracing::info!("Refreshed access token.");
                return Ok(new_creds.access_token().secret().to_string());
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

        let current_credentials = self
            .credentials
            .read()
            .await
            .clone()
            .ok_or_else(|| AuthError::AuthorizationRequired)?;

        let refresh_token = current_credentials.refresh_token().ok_or_else(|| {
            AuthError::TokenRefreshFailed("No refresh token available".to_string())
        })?;
        debug!("refresh token: {:?}", refresh_token);
        // refresh token
        let token_result = oauth_client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.secret().to_string()))
            .request_async(&self.http_client)
            .await
            .map_err(|e| AuthError::TokenRefreshFailed(e.to_string()))?;

        // store new credentials
        *self.credentials.write().await = Some(token_result.clone());

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

    async fn try_discover_oauth_server(
        &self,
        base_url: &Url,
    ) -> Result<Option<AuthorizationMetadata>, AuthError> {
        for candidate_path in Self::well_known_paths(base_url.path(), "oauth-authorization-server")
        {
            let mut discovery_url = base_url.clone();
            discovery_url.set_query(None);
            discovery_url.set_fragment(None);
            discovery_url.set_path(&candidate_path);
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
            .http_client
            .get(discovery_url.clone())
            .header("MCP-Protocol-Version", "2024-11-05")
            .send()
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

        let metadata = response
            .json::<AuthorizationMetadata>()
            .await
            .map_err(|e| AuthError::MetadataError(format!("Failed to parse metadata: {}", e)))?;
        debug!("metadata: {:?}", metadata);
        Ok(Some(metadata))
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
            .http_client
            .get(url.clone())
            .header("MCP-Protocol-Version", "2024-11-05")
            .send()
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
            .http_client
            .get(resource_metadata_url.clone())
            .header("MCP-Protocol-Version", "2024-11-05")
            .send()
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

        let metadata = response
            .json::<ResourceServerMetadata>()
            .await
            .map_err(|e| {
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
    ) -> Result<Self, AuthError> {
        // Default client config
        let config = OAuthClientConfig {
            client_id: "mcp-client".to_string(),
            client_secret: None,
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
            redirect_uri: redirect_uri.to_string(),
        };

        // try to dynamic register client
        let config = match auth_manager
            .register_client(client_name.unwrap_or("MCP Client"), redirect_uri)
            .await
        {
            Ok(config) => config,
            Err(e) => {
                warn!(
                    "Dynamic registration failed: {}, fallback to default config",
                    e
                );
                // fallback to default config
                config
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

            // write credentials
            *manager.credentials.write().await = Some(credentials);

            // discover metadata
            let metadata = manager.discover_metadata().await?;
            manager.metadata = Some(metadata);

            // set client id and secret
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
        if let OAuthState::Unauthorized(mut manager) = std::mem::replace(
            self,
            OAuthState::Unauthorized(AuthorizationManager::new(DEFAULT_EXCHANGE_URL).await?),
        ) {
            debug!("start discovery");
            let metadata = manager.discover_metadata().await?;
            manager.metadata = Some(metadata);
            debug!("start session");
            let session =
                AuthorizationSession::new(manager, scopes, redirect_uri, client_name).await?;
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
    use url::Url;

    use super::AuthorizationManager;

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
}
