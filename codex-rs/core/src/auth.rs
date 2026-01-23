mod storage;

use chrono::Utc;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
#[cfg(test)]
use serial_test::serial;
use std::env;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use codex_app_server_protocol::AuthMode;
use codex_protocol::config_types::ForcedLoginMethod;

pub use crate::auth::storage::AuthCredentialsStoreMode;
pub use crate::auth::storage::AuthDotJson;
use crate::auth::storage::AuthStorageBackend;
use crate::auth::storage::create_auth_storage;
use crate::config::Config;
use crate::error::RefreshTokenFailedError;
use crate::error::RefreshTokenFailedReason;
use crate::token_data::KnownPlan as InternalKnownPlan;
use crate::token_data::PlanType as InternalPlanType;
use crate::token_data::TokenData;
use crate::token_data::parse_id_token;
use crate::util::try_parse_error_message;
use codex_client::CodexHttpClient;
use codex_protocol::account::PlanType as AccountPlanType;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CodexAuth {
    pub mode: AuthMode,

    pub(crate) api_key: Option<String>,
    pub(crate) auth_dot_json: Arc<Mutex<Option<AuthDotJson>>>,
    storage: Arc<dyn AuthStorageBackend>,
    pub(crate) client: CodexHttpClient,
}

impl PartialEq for CodexAuth {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
    }
}

// TODO(pakrym): use token exp field to check for expiration instead
const TOKEN_REFRESH_INTERVAL: i64 = 8;

const REFRESH_TOKEN_EXPIRED_MESSAGE: &str = "Your access token could not be refreshed because your refresh token has expired. Please log out and sign in again.";
const REFRESH_TOKEN_REUSED_MESSAGE: &str = "Your access token could not be refreshed because your refresh token was already used. Please log out and sign in again.";
const REFRESH_TOKEN_INVALIDATED_MESSAGE: &str = "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.";
const REFRESH_TOKEN_UNKNOWN_MESSAGE: &str =
    "Your access token could not be refreshed. Please log out and sign in again.";
const REFRESH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_REFRESH_TOKEN_URL_OVERRIDE";

#[derive(Debug, Error)]
pub enum RefreshTokenError {
    #[error("{0}")]
    Permanent(#[from] RefreshTokenFailedError),
    #[error(transparent)]
    Transient(#[from] std::io::Error),
}

impl RefreshTokenError {
    pub fn failed_reason(&self) -> Option<RefreshTokenFailedReason> {
        match self {
            Self::Permanent(error) => Some(error.reason),
            Self::Transient(_) => None,
        }
    }
}

impl From<RefreshTokenError> for std::io::Error {
    fn from(err: RefreshTokenError) -> Self {
        match err {
            RefreshTokenError::Permanent(failed) => std::io::Error::other(failed),
            RefreshTokenError::Transient(inner) => inner,
        }
    }
}

impl CodexAuth {
    /// Loads the available auth information from auth storage.
    pub fn from_auth_storage(
        codex_home: &Path,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> std::io::Result<Option<CodexAuth>> {
        load_auth(codex_home, false, auth_credentials_store_mode)
    }

    pub fn get_token_data(&self) -> Result<TokenData, std::io::Error> {
        let auth_dot_json: Option<AuthDotJson> = self.get_current_auth_json();
        match auth_dot_json {
            Some(AuthDotJson {
                tokens: Some(tokens),
                last_refresh: Some(_),
                ..
            }) => Ok(tokens),
            _ => Err(std::io::Error::other("Token data is not available.")),
        }
    }

    pub fn get_token(&self) -> Result<String, std::io::Error> {
        match self.mode {
            AuthMode::ApiKey => Ok(self.api_key.clone().unwrap_or_default()),
            AuthMode::ChatGPT => {
                let id_token = self.get_token_data()?.access_token;
                Ok(id_token)
            }
        }
    }

    pub fn get_account_id(&self) -> Option<String> {
        self.get_current_token_data().and_then(|t| t.account_id)
    }

    pub fn get_account_email(&self) -> Option<String> {
        self.get_current_token_data().and_then(|t| t.id_token.email)
    }

    /// Account-facing plan classification derived from the current token.
    /// Returns a high-level `AccountPlanType` (e.g., Free/Plus/Pro/Team/…)
    /// mapped from the ID token's internal plan value. Prefer this when you
    /// need to make UI or product decisions based on the user's subscription.
    pub fn account_plan_type(&self) -> Option<AccountPlanType> {
        let map_known = |kp: &InternalKnownPlan| match kp {
            InternalKnownPlan::Free => AccountPlanType::Free,
            InternalKnownPlan::Plus => AccountPlanType::Plus,
            InternalKnownPlan::Pro => AccountPlanType::Pro,
            InternalKnownPlan::Team => AccountPlanType::Team,
            InternalKnownPlan::Business => AccountPlanType::Business,
            InternalKnownPlan::Enterprise => AccountPlanType::Enterprise,
            InternalKnownPlan::Edu => AccountPlanType::Edu,
        };

        self.get_current_token_data()
            .and_then(|t| t.id_token.chatgpt_plan_type)
            .map(|pt| match pt {
                InternalPlanType::Known(k) => map_known(&k),
                InternalPlanType::Unknown(_) => AccountPlanType::Unknown,
            })
    }

    fn get_current_auth_json(&self) -> Option<AuthDotJson> {
        #[expect(clippy::unwrap_used)]
        self.auth_dot_json.lock().unwrap().clone()
    }

    fn get_current_token_data(&self) -> Option<TokenData> {
        self.get_current_auth_json().and_then(|t| t.tokens)
    }

    /// Consider this private to integration tests.
    pub fn create_dummy_chatgpt_auth_for_testing() -> Self {
        let auth_dot_json = AuthDotJson {
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: Default::default(),
                access_token: "Access Token".to_string(),
                refresh_token: "test".to_string(),
                account_id: Some("account_id".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        };

        let auth_dot_json = Arc::new(Mutex::new(Some(auth_dot_json)));
        Self {
            api_key: None,
            mode: AuthMode::ChatGPT,
            storage: create_auth_storage(PathBuf::new(), AuthCredentialsStoreMode::File),
            auth_dot_json,
            client: crate::default_client::create_client(),
        }
    }

    fn from_api_key_with_client(api_key: &str, client: CodexHttpClient) -> Self {
        Self {
            api_key: Some(api_key.to_owned()),
            mode: AuthMode::ApiKey,
            storage: create_auth_storage(PathBuf::new(), AuthCredentialsStoreMode::File),
            auth_dot_json: Arc::new(Mutex::new(None)),
            client,
        }
    }

    pub fn from_api_key(api_key: &str) -> Self {
        Self::from_api_key_with_client(api_key, crate::default_client::create_client())
    }
}

pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
pub const CODEX_API_KEY_ENV_VAR: &str = "CODEX_API_KEY";

pub fn read_openai_api_key_from_env() -> Option<String> {
    env::var(OPENAI_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn read_codex_api_key_from_env() -> Option<String> {
    env::var(CODEX_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Delete the auth.json file inside `codex_home` if it exists. Returns `Ok(true)`
/// if a file was removed, `Ok(false)` if no auth file was present.
pub fn logout(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<bool> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    storage.delete()
}

/// Writes an `auth.json` that contains only the API key.
pub fn login_with_api_key(
    codex_home: &Path,
    api_key: &str,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        openai_api_key: Some(api_key.to_string()),
        tokens: None,
        last_refresh: None,
    };
    save_auth(codex_home, &auth_dot_json, auth_credentials_store_mode)
}

/// Persist the provided auth payload using the specified backend.
pub fn save_auth(
    codex_home: &Path,
    auth: &AuthDotJson,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    storage.save(auth)
}

/// Load CLI auth data using the configured credential store backend.
/// Returns `None` when no credentials are stored. This function is
/// provided only for tests. Production code should not directly load
/// from the auth.json storage. It should use the AuthManager abstraction
/// instead.
pub fn load_auth_dot_json(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<Option<AuthDotJson>> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    storage.load()
}

pub fn enforce_login_restrictions(config: &Config) -> std::io::Result<()> {
    let Some(auth) = load_auth(
        &config.codex_home,
        true,
        config.cli_auth_credentials_store_mode,
    )?
    else {
        return Ok(());
    };

    if let Some(required_method) = config.forced_login_method {
        let method_violation = match (required_method, auth.mode) {
            (ForcedLoginMethod::Api, AuthMode::ApiKey) => None,
            (ForcedLoginMethod::Chatgpt, AuthMode::ChatGPT) => None,
            (ForcedLoginMethod::Api, AuthMode::ChatGPT) => Some(
                "API key login is required, but ChatGPT is currently being used. Logging out."
                    .to_string(),
            ),
            (ForcedLoginMethod::Chatgpt, AuthMode::ApiKey) => Some(
                "ChatGPT login is required, but an API key is currently being used. Logging out."
                    .to_string(),
            ),
        };

        if let Some(message) = method_violation {
            return logout_with_message(
                &config.codex_home,
                message,
                config.cli_auth_credentials_store_mode,
            );
        }
    }

    if let Some(expected_account_id) = config.forced_chatgpt_workspace_id.as_deref() {
        if auth.mode != AuthMode::ChatGPT {
            return Ok(());
        }

        let token_data = match auth.get_token_data() {
            Ok(data) => data,
            Err(err) => {
                return logout_with_message(
                    &config.codex_home,
                    format!(
                        "Failed to load ChatGPT credentials while enforcing workspace restrictions: {err}. Logging out."
                    ),
                    config.cli_auth_credentials_store_mode,
                );
            }
        };

        // workspace is the external identifier for account id.
        let chatgpt_account_id = token_data.id_token.chatgpt_account_id.as_deref();
        if chatgpt_account_id != Some(expected_account_id) {
            let message = match chatgpt_account_id {
                Some(actual) => format!(
                    "Login is restricted to workspace {expected_account_id}, but current credentials belong to {actual}. Logging out."
                ),
                None => format!(
                    "Login is restricted to workspace {expected_account_id}, but current credentials lack a workspace identifier. Logging out."
                ),
            };
            return logout_with_message(
                &config.codex_home,
                message,
                config.cli_auth_credentials_store_mode,
            );
        }
    }

    Ok(())
}

fn logout_with_message(
    codex_home: &Path,
    message: String,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    match logout(codex_home, auth_credentials_store_mode) {
        Ok(_) => Err(std::io::Error::other(message)),
        Err(err) => Err(std::io::Error::other(format!(
            "{message}. Failed to remove auth.json: {err}"
        ))),
    }
}

fn load_auth(
    codex_home: &Path,
    enable_codex_api_key_env: bool,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<Option<CodexAuth>> {
    if enable_codex_api_key_env && let Some(api_key) = read_codex_api_key_from_env() {
        let client = crate::default_client::create_client();
        return Ok(Some(CodexAuth::from_api_key_with_client(
            api_key.as_str(),
            client,
        )));
    }

    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);

    let client = crate::default_client::create_client();
    let auth_dot_json = match storage.load()? {
        Some(auth) => auth,
        None => return Ok(None),
    };

    let AuthDotJson {
        openai_api_key: auth_json_api_key,
        tokens,
        last_refresh,
    } = auth_dot_json;

    // Prefer AuthMode.ApiKey if it's set in the auth.json.
    if let Some(api_key) = &auth_json_api_key {
        return Ok(Some(CodexAuth::from_api_key_with_client(api_key, client)));
    }

    Ok(Some(CodexAuth {
        api_key: None,
        mode: AuthMode::ChatGPT,
        storage: storage.clone(),
        auth_dot_json: Arc::new(Mutex::new(Some(AuthDotJson {
            openai_api_key: None,
            tokens,
            last_refresh,
        }))),
        client,
    }))
}

async fn update_tokens(
    storage: &Arc<dyn AuthStorageBackend>,
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> std::io::Result<AuthDotJson> {
    let mut auth_dot_json = storage
        .load()?
        .ok_or(std::io::Error::other("Token data is not available."))?;

    let tokens = auth_dot_json.tokens.get_or_insert_with(TokenData::default);
    if let Some(id_token) = id_token {
        tokens.id_token = parse_id_token(&id_token).map_err(std::io::Error::other)?;
    }
    if let Some(access_token) = access_token {
        tokens.access_token = access_token;
    }
    if let Some(refresh_token) = refresh_token {
        tokens.refresh_token = refresh_token;
    }
    auth_dot_json.last_refresh = Some(Utc::now());
    storage.save(&auth_dot_json)?;
    Ok(auth_dot_json)
}

async fn try_refresh_token(
    refresh_token: String,
    client: &CodexHttpClient,
) -> Result<RefreshResponse, RefreshTokenError> {
    let refresh_request = RefreshRequest {
        client_id: CLIENT_ID,
        grant_type: "refresh_token",
        refresh_token,
        scope: "openid profile email",
    };

    let endpoint = refresh_token_endpoint();

    // Use shared client factory to include standard headers
    let response = client
        .post(endpoint.as_str())
        .header("Content-Type", "application/json")
        .json(&refresh_request)
        .send()
        .await
        .map_err(|err| RefreshTokenError::Transient(std::io::Error::other(err)))?;

    let status = response.status();
    if status.is_success() {
        let refresh_response = response
            .json::<RefreshResponse>()
            .await
            .map_err(|err| RefreshTokenError::Transient(std::io::Error::other(err)))?;
        Ok(refresh_response)
    } else {
        let body = response.text().await.unwrap_or_default();
        tracing::error!("Failed to refresh token: {status}: {body}");
        if status == StatusCode::UNAUTHORIZED {
            let failed = classify_refresh_token_failure(&body);
            Err(RefreshTokenError::Permanent(failed))
        } else {
            let message = try_parse_error_message(&body);
            Err(RefreshTokenError::Transient(std::io::Error::other(
                format!("Failed to refresh token: {status}: {message}"),
            )))
        }
    }
}

fn classify_refresh_token_failure(body: &str) -> RefreshTokenFailedError {
    let code = extract_refresh_token_error_code(body);

    let normalized_code = code.as_deref().map(str::to_ascii_lowercase);
    let reason = match normalized_code.as_deref() {
        Some("refresh_token_expired") => RefreshTokenFailedReason::Expired,
        Some("refresh_token_reused") => RefreshTokenFailedReason::Exhausted,
        Some("refresh_token_invalidated") => RefreshTokenFailedReason::Revoked,
        _ => RefreshTokenFailedReason::Other,
    };

    if reason == RefreshTokenFailedReason::Other {
        tracing::warn!(
            backend_code = normalized_code.as_deref(),
            backend_body = body,
            "Encountered unknown 401 response while refreshing token"
        );
    }

    let message = match reason {
        RefreshTokenFailedReason::Expired => REFRESH_TOKEN_EXPIRED_MESSAGE.to_string(),
        RefreshTokenFailedReason::Exhausted => REFRESH_TOKEN_REUSED_MESSAGE.to_string(),
        RefreshTokenFailedReason::Revoked => REFRESH_TOKEN_INVALIDATED_MESSAGE.to_string(),
        RefreshTokenFailedReason::Other => REFRESH_TOKEN_UNKNOWN_MESSAGE.to_string(),
    };

    RefreshTokenFailedError::new(reason, message)
}

fn extract_refresh_token_error_code(body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return None;
    }

    let Value::Object(map) = serde_json::from_str::<Value>(body).ok()? else {
        return None;
    };

    if let Some(error_value) = map.get("error") {
        match error_value {
            Value::Object(obj) => {
                if let Some(code) = obj.get("code").and_then(Value::as_str) {
                    return Some(code.to_string());
                }
            }
            Value::String(code) => {
                return Some(code.to_string());
            }
            _ => {}
        }
    }

    map.get("code").and_then(Value::as_str).map(str::to_string)
}

#[derive(Serialize)]
struct RefreshRequest {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: String,
    scope: &'static str,
}

#[derive(Deserialize, Clone)]
struct RefreshResponse {
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

// Shared constant for token refresh (client id used for oauth token refresh flow)
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

fn refresh_token_endpoint() -> String {
    std::env::var(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR)
        .unwrap_or_else(|_| REFRESH_TOKEN_URL.to_string())
}

use std::sync::RwLock;

/// Internal cached auth state.
#[derive(Clone, Debug)]
struct CachedAuth {
    auth: Option<CodexAuth>,
}

enum UnauthorizedRecoveryStep {
    Reload,
    RefreshToken,
    Done,
}

enum ReloadOutcome {
    Reloaded,
    Skipped,
}

// UnauthorizedRecovery is a state machine that handles an attempt to refresh the authentication when requests
// to API fail with 401 status code.
// The client calls next() every time it encounters a 401 error, one time per retry.
// For API key based authentication, we don't do anything and let the error bubble to the user.
// For ChatGPT based authentication, we:
// 1. Attempt to reload the auth data from disk. We only reload if the account id matches the one the current process is running as.
// 2. Attempt to refresh the token using OAuth token refresh flow.
// If after both steps the server still responds with 401 we let the error bubble to the user.
pub struct UnauthorizedRecovery {
    manager: Arc<AuthManager>,
    step: UnauthorizedRecoveryStep,
    expected_account_id: Option<String>,
}

impl UnauthorizedRecovery {
    fn new(manager: Arc<AuthManager>) -> Self {
        let expected_account_id = manager
            .auth_cached()
            .as_ref()
            .and_then(CodexAuth::get_account_id);
        Self {
            manager,
            step: UnauthorizedRecoveryStep::Reload,
            expected_account_id,
        }
    }

    pub fn has_next(&self) -> bool {
        if !self
            .manager
            .auth_cached()
            .is_some_and(|auth| auth.mode == AuthMode::ChatGPT)
        {
            return false;
        }

        !matches!(self.step, UnauthorizedRecoveryStep::Done)
    }

    pub async fn next(&mut self) -> Result<(), RefreshTokenError> {
        if !self.has_next() {
            return Err(RefreshTokenError::Permanent(RefreshTokenFailedError::new(
                RefreshTokenFailedReason::Other,
                "No more recovery steps available.",
            )));
        }

        match self.step {
            UnauthorizedRecoveryStep::Reload => {
                match self
                    .manager
                    .reload_if_account_id_matches(self.expected_account_id.as_deref())
                {
                    ReloadOutcome::Reloaded => {
                        self.step = UnauthorizedRecoveryStep::RefreshToken;
                    }
                    ReloadOutcome::Skipped => {
                        self.manager.refresh_token().await?;
                        self.step = UnauthorizedRecoveryStep::Done;
                    }
                }
            }
            UnauthorizedRecoveryStep::RefreshToken => {
                self.manager.refresh_token().await?;
                self.step = UnauthorizedRecoveryStep::Done;
            }
            UnauthorizedRecoveryStep::Done => {}
        }
        Ok(())
    }
}

/// Central manager providing a single source of truth for auth.json derived
/// authentication data. It loads once (or on preference change) and then
/// hands out cloned `CodexAuth` values so the rest of the program has a
/// consistent snapshot.
///
/// External modifications to `auth.json` will NOT be observed until
/// `reload()` is called explicitly. This matches the design goal of avoiding
/// different parts of the program seeing inconsistent auth data mid‑run.
#[derive(Debug)]
pub struct AuthManager {
    codex_home: PathBuf,
    inner: RwLock<CachedAuth>,
    enable_codex_api_key_env: bool,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
}

impl AuthManager {
    /// Create a new manager loading the initial auth using the provided
    /// preferred auth method. Errors loading auth are swallowed; `auth()` will
    /// simply return `None` in that case so callers can treat it as an
    /// unauthenticated state.
    pub fn new(
        codex_home: PathBuf,
        enable_codex_api_key_env: bool,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> Self {
        let auth = load_auth(
            &codex_home,
            enable_codex_api_key_env,
            auth_credentials_store_mode,
        )
        .ok()
        .flatten();
        Self {
            codex_home,
            inner: RwLock::new(CachedAuth { auth }),
            enable_codex_api_key_env,
            auth_credentials_store_mode,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Create an AuthManager with a specific CodexAuth, for testing only.
    pub fn from_auth_for_testing(auth: CodexAuth) -> Arc<Self> {
        let cached = CachedAuth { auth: Some(auth) };

        Arc::new(Self {
            codex_home: PathBuf::from("non-existent"),
            inner: RwLock::new(cached),
            enable_codex_api_key_env: false,
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Create an AuthManager with a specific CodexAuth and codex home, for testing only.
    pub fn from_auth_for_testing_with_home(auth: CodexAuth, codex_home: PathBuf) -> Arc<Self> {
        let cached = CachedAuth { auth: Some(auth) };
        Arc::new(Self {
            codex_home,
            inner: RwLock::new(cached),
            enable_codex_api_key_env: false,
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        })
    }

    /// Current cached auth (clone) without attempting a refresh.
    pub fn auth_cached(&self) -> Option<CodexAuth> {
        self.inner.read().ok().and_then(|c| c.auth.clone())
    }

    /// Current cached auth (clone). May be `None` if not logged in or load failed.
    /// Refreshes cached ChatGPT tokens if they are stale before returning.
    pub async fn auth(&self) -> Option<CodexAuth> {
        let auth = self.auth_cached()?;
        if let Err(err) = self.refresh_if_stale(&auth).await {
            tracing::error!("Failed to refresh token: {}", err);
            return Some(auth);
        }
        self.auth_cached()
    }

    /// Force a reload of the auth information from auth.json. Returns
    /// whether the auth value changed.
    pub fn reload(&self) -> bool {
        tracing::info!("Reloading auth");
        let new_auth = self.load_auth_from_storage();
        self.set_auth(new_auth)
    }

    fn reload_if_account_id_matches(&self, expected_account_id: Option<&str>) -> ReloadOutcome {
        let expected_account_id = match expected_account_id {
            Some(account_id) => account_id,
            None => {
                tracing::info!("Skipping auth reload because no account id is available.");
                return ReloadOutcome::Skipped;
            }
        };

        let new_auth = self.load_auth_from_storage();
        let new_account_id = new_auth.as_ref().and_then(CodexAuth::get_account_id);

        if new_account_id.as_deref() != Some(expected_account_id) {
            let found_account_id = new_account_id.as_deref().unwrap_or("unknown");
            tracing::info!(
                "Skipping auth reload due to account id mismatch (expected: {expected_account_id}, found: {found_account_id})"
            );
            return ReloadOutcome::Skipped;
        }

        tracing::info!("Reloading auth for account {expected_account_id}");
        self.set_auth(new_auth);
        ReloadOutcome::Reloaded
    }

    fn auths_equal(a: &Option<CodexAuth>, b: &Option<CodexAuth>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    fn load_auth_from_storage(&self) -> Option<CodexAuth> {
        load_auth(
            &self.codex_home,
            self.enable_codex_api_key_env,
            self.auth_credentials_store_mode,
        )
        .ok()
        .flatten()
    }

    fn set_auth(&self, new_auth: Option<CodexAuth>) -> bool {
        if let Ok(mut guard) = self.inner.write() {
            let changed = !AuthManager::auths_equal(&guard.auth, &new_auth);
            tracing::info!("Reloaded auth, changed: {changed}");
            guard.auth = new_auth;
            changed
        } else {
            false
        }
    }

    /// Convenience constructor returning an `Arc` wrapper.
    pub fn shared(
        codex_home: PathBuf,
        enable_codex_api_key_env: bool,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> Arc<Self> {
        Arc::new(Self::new(
            codex_home,
            enable_codex_api_key_env,
            auth_credentials_store_mode,
        ))
    }

    pub fn unauthorized_recovery(self: &Arc<Self>) -> UnauthorizedRecovery {
        UnauthorizedRecovery::new(Arc::clone(self))
    }

    /// Attempt to refresh the current auth token (if any). On success, reload
    /// the auth state from disk so other components observe refreshed token.
    /// If the token refresh fails, returns the error to the caller.
    pub async fn refresh_token(&self) -> Result<(), RefreshTokenError> {
        tracing::info!("Refreshing token");

        let auth = match self.auth_cached() {
            Some(auth) => auth,
            None => return Ok(()),
        };
        let token_data = auth.get_current_token_data().ok_or_else(|| {
            RefreshTokenError::Transient(std::io::Error::other("Token data is not available."))
        })?;
        self.refresh_tokens(&auth, token_data.refresh_token).await?;
        // Reload to pick up persisted changes.
        self.reload();
        Ok(())
    }

    /// Log out by deleting the on‑disk auth.json (if present). Returns Ok(true)
    /// if a file was removed, Ok(false) if no auth file existed. On success,
    /// reloads the in‑memory auth cache so callers immediately observe the
    /// unauthenticated state.
    pub fn logout(&self) -> std::io::Result<bool> {
        let removed = super::auth::logout(&self.codex_home, self.auth_credentials_store_mode)?;
        // Always reload to clear any cached auth (even if file absent).
        self.reload();
        Ok(removed)
    }

    pub fn get_auth_mode(&self) -> Option<AuthMode> {
        self.auth_cached().map(|a| a.mode)
    }

    async fn refresh_if_stale(&self, auth: &CodexAuth) -> Result<bool, RefreshTokenError> {
        if auth.mode != AuthMode::ChatGPT {
            return Ok(false);
        }

        let auth_dot_json = match auth.get_current_auth_json() {
            Some(auth_dot_json) => auth_dot_json,
            None => return Ok(false),
        };
        let tokens = match auth_dot_json.tokens {
            Some(tokens) => tokens,
            None => return Ok(false),
        };
        let last_refresh = match auth_dot_json.last_refresh {
            Some(last_refresh) => last_refresh,
            None => return Ok(false),
        };
        if last_refresh >= Utc::now() - chrono::Duration::days(TOKEN_REFRESH_INTERVAL) {
            return Ok(false);
        }
        self.refresh_tokens(auth, tokens.refresh_token).await?;
        self.reload();
        Ok(true)
    }

    async fn refresh_tokens(
        &self,
        auth: &CodexAuth,
        refresh_token: String,
    ) -> Result<(), RefreshTokenError> {
        let refresh_response = try_refresh_token(refresh_token, &auth.client).await?;

        update_tokens(
            &auth.storage,
            refresh_response.id_token,
            refresh_response.access_token,
            refresh_response.refresh_token,
        )
        .await
        .map_err(RefreshTokenError::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::storage::FileAuthStorage;
    use crate::auth::storage::get_auth_file;
    use crate::config::Config;
    use crate::config::ConfigBuilder;
    use crate::token_data::IdTokenInfo;
    use crate::token_data::KnownPlan as InternalKnownPlan;
    use crate::token_data::PlanType as InternalPlanType;
    use codex_protocol::account::PlanType as AccountPlanType;

    use base64::Engine;
    use codex_protocol::config_types::ForcedLoginMethod;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn refresh_without_id_token() {
        let codex_home = tempdir().unwrap();
        let fake_jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: None,
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let storage = create_auth_storage(
            codex_home.path().to_path_buf(),
            AuthCredentialsStoreMode::File,
        );
        let updated = super::update_tokens(
            &storage,
            None,
            Some("new-access-token".to_string()),
            Some("new-refresh-token".to_string()),
        )
        .await
        .expect("update_tokens should succeed");

        let tokens = updated.tokens.expect("tokens should exist");
        assert_eq!(tokens.id_token.raw_jwt, fake_jwt);
        assert_eq!(tokens.access_token, "new-access-token");
        assert_eq!(tokens.refresh_token, "new-refresh-token");
    }

    #[test]
    fn login_with_api_key_overwrites_existing_auth_json() {
        let dir = tempdir().unwrap();
        let auth_path = dir.path().join("auth.json");
        let stale_auth = json!({
            "OPENAI_API_KEY": "sk-old",
            "tokens": {
                "id_token": "stale.header.payload",
                "access_token": "stale-access",
                "refresh_token": "stale-refresh",
                "account_id": "stale-acc"
            }
        });
        std::fs::write(
            &auth_path,
            serde_json::to_string_pretty(&stale_auth).unwrap(),
        )
        .unwrap();

        super::login_with_api_key(dir.path(), "sk-new", AuthCredentialsStoreMode::File)
            .expect("login_with_api_key should succeed");

        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let auth = storage
            .try_read_auth_json(&auth_path)
            .expect("auth.json should parse");
        assert_eq!(auth.openai_api_key.as_deref(), Some("sk-new"));
        assert!(auth.tokens.is_none(), "tokens should be cleared");
    }

    #[test]
    fn missing_auth_json_returns_none() {
        let dir = tempdir().unwrap();
        let auth = CodexAuth::from_auth_storage(dir.path(), AuthCredentialsStoreMode::File)
            .expect("call should succeed");
        assert_eq!(auth, None);
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn pro_account_with_no_api_key_uses_chatgpt_auth() {
        let codex_home = tempdir().unwrap();
        let fake_jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: None,
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let CodexAuth {
            api_key,
            mode,
            auth_dot_json,
            storage: _,
            ..
        } = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
            .unwrap()
            .unwrap();
        assert_eq!(None, api_key);
        assert_eq!(AuthMode::ChatGPT, mode);

        let guard = auth_dot_json.lock().unwrap();
        let auth_dot_json = guard.as_ref().expect("AuthDotJson should exist");
        let last_refresh = auth_dot_json
            .last_refresh
            .expect("last_refresh should be recorded");

        assert_eq!(
            &AuthDotJson {
                openai_api_key: None,
                tokens: Some(TokenData {
                    id_token: IdTokenInfo {
                        email: Some("user@example.com".to_string()),
                        chatgpt_plan_type: Some(InternalPlanType::Known(InternalKnownPlan::Pro)),
                        chatgpt_user_id: Some("user-12345".to_string()),
                        chatgpt_account_id: None,
                        raw_jwt: fake_jwt,
                    },
                    access_token: "test-access-token".to_string(),
                    refresh_token: "test-refresh-token".to_string(),
                    account_id: None,
                }),
                last_refresh: Some(last_refresh),
            },
            auth_dot_json
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn loads_api_key_from_auth_json() {
        let dir = tempdir().unwrap();
        let auth_file = dir.path().join("auth.json");
        std::fs::write(
            auth_file,
            r#"{"OPENAI_API_KEY":"sk-test-key","tokens":null,"last_refresh":null}"#,
        )
        .unwrap();

        let auth = super::load_auth(dir.path(), false, AuthCredentialsStoreMode::File)
            .unwrap()
            .unwrap();
        assert_eq!(auth.mode, AuthMode::ApiKey);
        assert_eq!(auth.api_key, Some("sk-test-key".to_string()));

        assert!(auth.get_token_data().is_err());
    }

    #[test]
    fn logout_removes_auth_file() -> Result<(), std::io::Error> {
        let dir = tempdir()?;
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("sk-test-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        super::save_auth(dir.path(), &auth_dot_json, AuthCredentialsStoreMode::File)?;
        let auth_file = get_auth_file(dir.path());
        assert!(auth_file.exists());
        assert!(logout(dir.path(), AuthCredentialsStoreMode::File)?);
        assert!(!auth_file.exists());
        Ok(())
    }

    struct AuthFileParams {
        openai_api_key: Option<String>,
        chatgpt_plan_type: String,
        chatgpt_account_id: Option<String>,
    }

    fn write_auth_file(params: AuthFileParams, codex_home: &Path) -> std::io::Result<String> {
        let auth_file = get_auth_file(codex_home);
        // Create a minimal valid JWT for the id_token field.
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let mut auth_payload = serde_json::json!({
            "chatgpt_plan_type": params.chatgpt_plan_type,
            "chatgpt_user_id": "user-12345",
            "user_id": "user-12345",
        });

        if let Some(chatgpt_account_id) = params.chatgpt_account_id {
            let org_value = serde_json::Value::String(chatgpt_account_id);
            auth_payload["chatgpt_account_id"] = org_value;
        }

        let payload = serde_json::json!({
            "email": "user@example.com",
            "email_verified": true,
            "https://api.openai.com/auth": auth_payload,
        });
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let header_b64 = b64(&serde_json::to_vec(&header)?);
        let payload_b64 = b64(&serde_json::to_vec(&payload)?);
        let signature_b64 = b64(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let auth_json_data = json!({
            "OPENAI_API_KEY": params.openai_api_key,
            "tokens": {
                "id_token": fake_jwt,
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token"
            },
            "last_refresh": Utc::now(),
        });
        let auth_json = serde_json::to_string_pretty(&auth_json_data)?;
        std::fs::write(auth_file, auth_json)?;
        Ok(fake_jwt)
    }

    async fn build_config(
        codex_home: &Path,
        forced_login_method: Option<ForcedLoginMethod>,
        forced_chatgpt_workspace_id: Option<String>,
    ) -> Config {
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.to_path_buf())
            .build()
            .await
            .expect("config should load");
        config.forced_login_method = forced_login_method;
        config.forced_chatgpt_workspace_id = forced_chatgpt_workspace_id;
        config
    }

    /// Use sparingly.
    /// TODO (gpeal): replace this with an injectable env var provider.
    #[cfg(test)]
    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    #[cfg(test)]
    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    #[cfg(test)]
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => env::set_var(self.key, value),
                    None => env::remove_var(self.key),
                }
            }
        }
    }

    #[tokio::test]
    async fn enforce_login_restrictions_logs_out_for_method_mismatch() {
        let codex_home = tempdir().unwrap();
        login_with_api_key(codex_home.path(), "sk-test", AuthCredentialsStoreMode::File)
            .expect("seed api key");

        let config = build_config(codex_home.path(), Some(ForcedLoginMethod::Chatgpt), None).await;

        let err = super::enforce_login_restrictions(&config)
            .expect_err("expected method mismatch to error");
        assert!(err.to_string().contains("ChatGPT login is required"));
        assert!(
            !codex_home.path().join("auth.json").exists(),
            "auth.json should be removed on mismatch"
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn enforce_login_restrictions_logs_out_for_workspace_mismatch() {
        let codex_home = tempdir().unwrap();
        let _jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: Some("org_another_org".to_string()),
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let config = build_config(codex_home.path(), None, Some("org_mine".to_string())).await;

        let err = super::enforce_login_restrictions(&config)
            .expect_err("expected workspace mismatch to error");
        assert!(err.to_string().contains("workspace org_mine"));
        assert!(
            !codex_home.path().join("auth.json").exists(),
            "auth.json should be removed on mismatch"
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn enforce_login_restrictions_allows_matching_workspace() {
        let codex_home = tempdir().unwrap();
        let _jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: Some("org_mine".to_string()),
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let config = build_config(codex_home.path(), None, Some("org_mine".to_string())).await;

        super::enforce_login_restrictions(&config).expect("matching workspace should succeed");
        assert!(
            codex_home.path().join("auth.json").exists(),
            "auth.json should remain when restrictions pass"
        );
    }

    #[tokio::test]
    async fn enforce_login_restrictions_allows_api_key_if_login_method_not_set_but_forced_chatgpt_workspace_id_is_set()
     {
        let codex_home = tempdir().unwrap();
        login_with_api_key(codex_home.path(), "sk-test", AuthCredentialsStoreMode::File)
            .expect("seed api key");

        let config = build_config(codex_home.path(), None, Some("org_mine".to_string())).await;

        super::enforce_login_restrictions(&config).expect("matching workspace should succeed");
        assert!(
            codex_home.path().join("auth.json").exists(),
            "auth.json should remain when restrictions pass"
        );
    }

    #[tokio::test]
    #[serial(codex_api_key)]
    async fn enforce_login_restrictions_blocks_env_api_key_when_chatgpt_required() {
        let _guard = EnvVarGuard::set(CODEX_API_KEY_ENV_VAR, "sk-env");
        let codex_home = tempdir().unwrap();

        let config = build_config(codex_home.path(), Some(ForcedLoginMethod::Chatgpt), None).await;

        let err = super::enforce_login_restrictions(&config)
            .expect_err("environment API key should not satisfy forced ChatGPT login");
        assert!(
            err.to_string()
                .contains("ChatGPT login is required, but an API key is currently being used.")
        );
    }

    #[test]
    fn plan_type_maps_known_plan() {
        let codex_home = tempdir().unwrap();
        let _jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "pro".to_string(),
                chatgpt_account_id: None,
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let auth = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
            .expect("load auth")
            .expect("auth available");

        pretty_assertions::assert_eq!(auth.account_plan_type(), Some(AccountPlanType::Pro));
    }

    #[test]
    fn plan_type_maps_unknown_to_unknown() {
        let codex_home = tempdir().unwrap();
        let _jwt = write_auth_file(
            AuthFileParams {
                openai_api_key: None,
                chatgpt_plan_type: "mystery-tier".to_string(),
                chatgpt_account_id: None,
            },
            codex_home.path(),
        )
        .expect("failed to write auth file");

        let auth = super::load_auth(codex_home.path(), false, AuthCredentialsStoreMode::File)
            .expect("load auth")
            .expect("auth available");

        pretty_assertions::assert_eq!(auth.account_plan_type(), Some(AccountPlanType::Unknown));
    }
}
