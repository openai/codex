mod storage;

use chrono::DateTime;
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
use std::time::Duration;
use std::time::Instant;

use codex_app_server_protocol::AuthMode;
use codex_protocol::config_types::ForcedLoginMethod;

pub use crate::auth::storage::AuthCredentialsStoreMode;
pub use crate::auth::storage::AuthDotJson;
use crate::auth::storage::AuthProviderEntry;
use crate::auth::storage::AuthStorageBackend;
use crate::auth::storage::AuthStore;
pub(crate) use crate::auth::storage::DEFAULT_OAUTH_NAMESPACE;
use crate::auth::storage::DEFAULT_OAUTH_PROVIDER_ID;
use crate::auth::storage::OAuthHealth;
use crate::auth::storage::OAuthProvider;
use crate::auth::storage::OAuthRecord;
use crate::auth::storage::auth_dot_json_from_store;
use crate::auth::storage::auth_store_from_legacy;
use crate::auth::storage::create_auth_storage;
use crate::auth::storage::normalize_oauth_namespace;
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
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CodexAuth {
    pub mode: AuthMode,

    pub(crate) api_key: Option<String>,
    pub(crate) oauth_record_id: Option<String>,
    pub(crate) auth_dot_json: Arc<Mutex<Option<AuthDotJson>>>,
    storage: Arc<dyn AuthStorageBackend>,
    pub(crate) client: CodexHttpClient,
}

impl PartialEq for CodexAuth {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.api_key == other.api_key
            && self.oauth_record_id == other.oauth_record_id
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

const STORE_LOCK_TIMEOUT_MS: u64 = 5_000;
const STORE_LOCK_STALE_MS: u64 = 30_000;
const STORE_LOCK_RETRY_MS: u64 = 25;
const STORE_LOCK_BEST_EFFORT_TIMEOUT_MS: u64 = 250;
const STORE_LOCK_BEST_EFFORT_RETRY_MS: u64 = 10;

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

#[derive(Clone, Copy)]
struct StoreLockOptions {
    timeout: Duration,
    stale: Duration,
    retry: Duration,
}

fn default_lock_options() -> StoreLockOptions {
    StoreLockOptions {
        timeout: Duration::from_millis(STORE_LOCK_TIMEOUT_MS),
        stale: Duration::from_millis(STORE_LOCK_STALE_MS),
        retry: Duration::from_millis(STORE_LOCK_RETRY_MS),
    }
}

fn best_effort_lock_options() -> StoreLockOptions {
    StoreLockOptions {
        timeout: Duration::from_millis(STORE_LOCK_BEST_EFFORT_TIMEOUT_MS),
        stale: Duration::from_millis(STORE_LOCK_STALE_MS),
        retry: Duration::from_millis(STORE_LOCK_BEST_EFFORT_RETRY_MS),
    }
}

struct StoreLock {
    path: PathBuf,
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn store_lock_path(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json.lock")
}

fn acquire_store_lock(codex_home: &Path, options: StoreLockOptions) -> std::io::Result<StoreLock> {
    let lock_path = store_lock_path(codex_home);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let start = Instant::now();
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(file) => {
                drop(file);
                return Ok(StoreLock { path: lock_path });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = std::fs::metadata(&lock_path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok())
                    .map(|elapsed| elapsed > options.stale)
                    .unwrap_or(false);
                if stale {
                    let _ = std::fs::remove_file(&lock_path);
                    continue;
                }
                if start.elapsed() > options.timeout {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "timed out waiting for auth store lock",
                    ));
                }
                std::thread::sleep(options.retry);
            }
            Err(err) => return Err(err),
        }
    }
}

fn with_store_lock<T>(
    codex_home: &Path,
    options: StoreLockOptions,
    f: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<T> {
    let _lock = acquire_store_lock(codex_home, options)?;
    f()
}

fn update_auth_store<T>(
    codex_home: &Path,
    storage: &Arc<dyn AuthStorageBackend>,
    options: StoreLockOptions,
    f: impl FnOnce(&mut AuthStore) -> std::io::Result<(T, bool)>,
) -> std::io::Result<T> {
    with_store_lock(codex_home, options, || {
        let mut store = storage.load()?.unwrap_or_default();
        let (value, changed) = f(&mut store)?;
        if changed {
            storage.save(&store)?;
        }
        Ok(value)
    })
}

fn update_auth_store_best_effort(
    codex_home: &Path,
    storage: &Arc<dyn AuthStorageBackend>,
    f: impl FnOnce(&mut AuthStore) -> std::io::Result<bool>,
) -> std::io::Result<()> {
    match update_auth_store(codex_home, storage, best_effort_lock_options(), |store| {
        let changed = f(store)?;
        Ok(((), changed))
    }) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::TimedOut => Ok(()),
        Err(err) => Err(err),
    }
}

fn ensure_oauth_provider<'a>(store: &'a mut AuthStore, provider_id: &str) -> &'a mut OAuthProvider {
    let entry = store
        .providers
        .entry(provider_id.to_string())
        .or_insert_with(|| AuthProviderEntry::Oauth(OAuthProvider::default()));
    match entry {
        AuthProviderEntry::Oauth(provider) => provider,
        AuthProviderEntry::Api { .. } => {
            *entry = AuthProviderEntry::Oauth(OAuthProvider::default());
            match entry {
                AuthProviderEntry::Oauth(provider) => provider,
                _ => unreachable!("just replaced with oauth provider"),
            }
        }
    }
}

fn normalize_record_order(ids: &[String], order: &[String]) -> Vec<String> {
    let mut ordered: Vec<String> = Vec::new();
    for id in order {
        if ids.iter().any(|candidate| candidate == id) && !ordered.contains(id) {
            ordered.push(id.clone());
        }
    }
    for id in ids {
        if !ordered.contains(id) {
            ordered.push(id.clone());
        }
    }
    ordered
}

fn record_ids_for_namespace(provider: &OAuthProvider, namespace: &str) -> Vec<String> {
    let ids: Vec<String> = provider
        .records
        .iter()
        .filter(|record| record.namespace == namespace)
        .map(|record| record.id.clone())
        .collect();
    let order = provider.order.get(namespace).cloned().unwrap_or_default();
    normalize_record_order(&ids, &order)
}

fn active_record_id(provider: &OAuthProvider, namespace: &str) -> Option<String> {
    provider
        .active
        .get(namespace)
        .cloned()
        .or_else(|| provider.order.get(namespace).and_then(|order| order.first().cloned()))
}

fn find_record_mut<'a>(provider: &'a mut OAuthProvider, record_id: &str) -> Option<&'a mut OAuthRecord> {
    provider.records.iter_mut().find(|record| record.id == record_id)
}

fn find_record<'a>(provider: &'a OAuthProvider, record_id: &str) -> Option<&'a OAuthRecord> {
    provider.records.iter().find(|record| record.id == record_id)
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
            oauth_record_id: Some("test-record".to_string()),
            storage: create_auth_storage(PathBuf::new(), AuthCredentialsStoreMode::File),
            auth_dot_json,
            client: crate::default_client::create_client(),
        }
    }

    fn from_api_key_with_client(api_key: &str, client: CodexHttpClient) -> Self {
        Self {
            api_key: Some(api_key.to_owned()),
            mode: AuthMode::ApiKey,
            oauth_record_id: None,
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
    set_openai_api_key(
        codex_home,
        auth_credentials_store_mode,
        Some(api_key.to_string()),
    )
}

/// Persist the provided auth payload using the specified backend.
pub fn save_auth(
    codex_home: &Path,
    auth: &AuthDotJson,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    let store = auth_store_from_legacy(auth.clone());
    with_store_lock(codex_home, default_lock_options(), || storage.save(&store))
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
    let Some(store) = storage.load()? else {
        return Ok(None);
    };

    let oauth_view = auth_dot_json_from_store(&store, DEFAULT_OAUTH_PROVIDER_ID, DEFAULT_OAUTH_NAMESPACE);
    let (tokens, last_refresh) = match oauth_view {
        Some(view) => (view.tokens, view.last_refresh),
        None => (None, None),
    };

    Ok(Some(AuthDotJson {
        openai_api_key: store.openai_api_key,
        tokens,
        last_refresh,
    }))
}

#[derive(Debug, Clone)]
pub struct OAuthHealthSummary {
    pub cooldown_until: Option<DateTime<Utc>>,
    pub exhausted_until: Option<DateTime<Utc>>,
    pub requires_relogin: bool,
    pub last_status_code: Option<u16>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Clone)]
pub struct OAuthAccountSummary {
    pub record_id: String,
    pub namespace: String,
    pub label: Option<String>,
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub last_refresh: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub health: OAuthHealthSummary,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct OAuthRotationSummary {
    pub total: usize,
    pub ready: usize,
    pub cooldown: usize,
    pub exhausted: usize,
    pub requires_relogin: usize,
}

impl OAuthRotationSummary {
    pub fn format_compact(&self) -> String {
        format!(
            "{}/{} ready, {} cooldown, {} exhausted, {} relogin",
            self.ready, self.total, self.cooldown, self.exhausted, self.requires_relogin
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OAuthPoolRecord {
    pub id: String,
    pub health: OAuthHealth,
}

#[derive(Debug, Clone)]
pub(crate) struct OAuthPoolSnapshot {
    pub records: Vec<OAuthPoolRecord>,
    pub ordered_ids: Vec<String>,
}

fn record_account_id(record: &OAuthRecord) -> Option<&str> {
    record
        .tokens
        .account_id
        .as_deref()
        .or(record.tokens.id_token.chatgpt_account_id.as_deref())
}

fn record_email(record: &OAuthRecord) -> Option<&str> {
    record.tokens.id_token.email.as_deref()
}

fn unique_record_match<F>(
    provider: &OAuthProvider,
    namespace: &str,
    mut predicate: F,
) -> Option<String>
where
    F: FnMut(&OAuthRecord) -> bool,
{
    let mut matched: Option<&OAuthRecord> = None;
    for record in provider.records.iter() {
        if record.namespace != namespace || !predicate(record) {
            continue;
        }
        if matched.is_some() {
            return None;
        }
        matched = Some(record);
    }
    matched.map(|record| record.id.clone())
}

pub fn add_oauth_account(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    mut tokens: TokenData,
    last_refresh: Option<DateTime<Utc>>,
    label: Option<String>,
) -> std::io::Result<String> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    let namespace = DEFAULT_OAUTH_NAMESPACE.to_string();
    update_auth_store(codex_home, &storage, default_lock_options(), |store| {
        let provider = ensure_oauth_provider(store, DEFAULT_OAUTH_PROVIDER_ID);
        let now = Utc::now();

        if tokens.account_id.is_none() {
            tokens.account_id = tokens.id_token.chatgpt_account_id.clone();
        }
        let account_id = tokens.account_id.as_deref();
        let email = tokens.id_token.email.as_deref();

        let existing_id = provider
            .records
            .iter()
            .find(|record| record.namespace == namespace && record.tokens.refresh_token == tokens.refresh_token)
            .map(|record| record.id.clone());
        let existing_id = existing_id
            .or_else(|| {
                account_id.and_then(|id| {
                    unique_record_match(provider, &namespace, |record| {
                        record_account_id(record) == Some(id)
                    })
                })
            })
            .or_else(|| {
                email.and_then(|email| {
                    unique_record_match(provider, &namespace, |record| {
                        record_email(record) == Some(email)
                    })
                })
            });

        let record_id = if let Some(existing_id) = existing_id {
            if let Some(record) = find_record_mut(provider, &existing_id) {
                record.tokens = tokens;
                record.last_refresh = last_refresh.or(Some(now));
                record.updated_at = now;
                record.health.requires_relogin = false;
                if label.is_some() {
                    record.label = label;
                }
            }
            existing_id
        } else {
            let record_id = Uuid::new_v4().to_string();
            provider.records.push(OAuthRecord {
                id: record_id.clone(),
                namespace: namespace.clone(),
                label: label.or_else(|| tokens.id_token.email.clone()),
                tokens,
                last_refresh: last_refresh.or(Some(now)),
                created_at: now,
                updated_at: now,
                health: OAuthHealth::default(),
            });
            record_id
        };

        let order = provider.order.entry(namespace.clone()).or_default();
        if !order.contains(&record_id) {
            order.push(record_id.clone());
        }
        provider.active.insert(namespace.clone(), record_id.clone());

        Ok((record_id, true))
    })
}

pub fn set_openai_api_key(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    api_key: Option<String>,
) -> std::io::Result<()> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    update_auth_store(codex_home, &storage, default_lock_options(), |store| {
        let changed = store.openai_api_key != api_key;
        store.openai_api_key = api_key;
        Ok(((), changed))
    })
}

pub fn list_oauth_accounts(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<Vec<OAuthAccountSummary>> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    let store = storage.load()?.unwrap_or_default();
    let provider = match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
        Some(AuthProviderEntry::Oauth(provider)) => provider,
        _ => return Ok(Vec::new()),
    };

    let namespace = DEFAULT_OAUTH_NAMESPACE;
    let ordered = record_ids_for_namespace(provider, namespace);
    let active = active_record_id(provider, namespace);

    let mut out = Vec::new();
    for id in ordered {
        if let Some(record) = find_record(provider, &id) {
            let health = OAuthHealthSummary {
                cooldown_until: record.health.cooldown_until,
                exhausted_until: record.health.exhausted_until,
                requires_relogin: record.health.requires_relogin,
                last_status_code: record.health.last_status_code,
                last_error_at: record.health.last_error_at,
                success_count: record.health.success_count,
                failure_count: record.health.failure_count,
            };
            out.push(OAuthAccountSummary {
                record_id: record.id.clone(),
                namespace: record.namespace.clone(),
                label: record.label.clone(),
                email: record.tokens.id_token.email.clone(),
                account_id: record.tokens.account_id.clone(),
                last_refresh: record.last_refresh,
                created_at: record.created_at,
                updated_at: record.updated_at,
                health,
                active: Some(record.id.clone()) == active,
            });
        }
    }

    Ok(out)
}

pub fn remove_oauth_account(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    record_id: &str,
) -> std::io::Result<bool> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    update_auth_store(codex_home, &storage, default_lock_options(), |store| {
        let mut remove_provider = false;
        let changed = match store.providers.get_mut(DEFAULT_OAUTH_PROVIDER_ID) {
            Some(AuthProviderEntry::Oauth(provider)) => {
                let before = provider.records.len();
                provider.records.retain(|record| record.id != record_id);
                if provider.records.len() == before {
                    return Ok((false, false));
                }

                let mut namespaces: Vec<String> = provider
                    .order
                    .keys()
                    .chain(provider.active.keys())
                    .map(|ns| ns.to_string())
                    .collect();
                namespaces.sort();
                namespaces.dedup();

                for ns in namespaces {
                    let ids = record_ids_for_namespace(provider, &ns);
                    if ids.is_empty() {
                        provider.order.remove(&ns);
                        provider.active.remove(&ns);
                        continue;
                    }
                    provider.order.insert(ns.clone(), ids.clone());
                    let next_active = match provider.active.get(&ns) {
                        Some(active_id) if ids.contains(active_id) => active_id.clone(),
                        _ => ids[0].clone(),
                    };
                    provider.active.insert(ns.clone(), next_active);
                }

                if provider.records.is_empty() {
                    remove_provider = true;
                }
                true
            }
            _ => return Ok((false, false)),
        };

        if remove_provider {
            store.providers.remove(DEFAULT_OAUTH_PROVIDER_ID);
        }
        Ok((changed, changed))
    })
}

pub fn remove_all_oauth_accounts(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<bool> {
    let storage = create_auth_storage(codex_home.to_path_buf(), auth_credentials_store_mode);
    update_auth_store(codex_home, &storage, default_lock_options(), |store| {
        let removed = store.providers.remove(DEFAULT_OAUTH_PROVIDER_ID).is_some();
        Ok((removed, removed))
    })
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

fn chatgpt_auth_from_record(
    storage: Arc<dyn AuthStorageBackend>,
    record: &OAuthRecord,
) -> CodexAuth {
    CodexAuth {
        api_key: None,
        mode: AuthMode::ChatGPT,
        oauth_record_id: Some(record.id.clone()),
        storage,
        auth_dot_json: Arc::new(Mutex::new(Some(AuthDotJson {
            openai_api_key: None,
            tokens: Some(record.tokens.clone()),
            last_refresh: record.last_refresh,
        }))),
        client: crate::default_client::create_client(),
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
    let store = match storage.load()? {
        Some(store) => store,
        None => return Ok(None),
    };

    // Prefer AuthMode.ApiKey if it's set in the auth store.
    if let Some(api_key) = &store.openai_api_key {
        return Ok(Some(CodexAuth::from_api_key_with_client(api_key, client)));
    }

    let provider = match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
        Some(AuthProviderEntry::Oauth(provider)) => provider,
        _ => return Ok(None),
    };

    let namespace = DEFAULT_OAUTH_NAMESPACE;
    let record_id = match active_record_id(provider, namespace) {
        Some(id) => id,
        None => return Ok(None),
    };
    let record = match find_record(provider, &record_id) {
        Some(record) => record,
        None => return Ok(None),
    };

    Ok(Some(chatgpt_auth_from_record(storage.clone(), record)))
}

async fn update_tokens(
    codex_home: &Path,
    storage: &Arc<dyn AuthStorageBackend>,
    record_id: Option<&str>,
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> std::io::Result<AuthDotJson> {
    update_auth_store(codex_home, storage, default_lock_options(), |store| {
        let openai_api_key = store.openai_api_key.clone();
        let (tokens, last_refresh) = {
            let provider = ensure_oauth_provider(store, DEFAULT_OAUTH_PROVIDER_ID);
            let namespace = DEFAULT_OAUTH_NAMESPACE;
            let mut target_id = record_id.map(|id| id.to_string());
            if let Some(ref id) = target_id {
                if !provider.records.iter().any(|record| record.id == *id) {
                    target_id = None;
                }
            }

            if target_id.is_none() {
                if let Some(refresh) = refresh_token.as_deref() {
                    target_id = provider
                        .records
                        .iter()
                        .find(|record| {
                            record.namespace == namespace && record.tokens.refresh_token == refresh
                        })
                        .map(|record| record.id.clone());
                }
            }

            if target_id.is_none() {
                target_id = active_record_id(provider, namespace);
            }

            let target_id =
                target_id.ok_or_else(|| std::io::Error::other("Token data is not available."))?;
            let record = find_record_mut(provider, &target_id)
                .ok_or_else(|| std::io::Error::other("Token data is not available."))?;

            if let Some(id_token) = id_token {
                record.tokens.id_token = parse_id_token(&id_token).map_err(std::io::Error::other)?;
            }
            if let Some(access_token) = access_token {
                record.tokens.access_token = access_token;
            }
            if let Some(refresh_token) = refresh_token {
                record.tokens.refresh_token = refresh_token;
            }
            record.last_refresh = Some(Utc::now());
            record.updated_at = Utc::now();
            (record.tokens.clone(), record.last_refresh)
        };

        Ok((
            AuthDotJson {
                openai_api_key,
                tokens: Some(tokens),
                last_refresh,
            },
            true,
        ))
    })
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

    fn auth_storage(&self) -> Arc<dyn AuthStorageBackend> {
        create_auth_storage(self.codex_home.clone(), self.auth_credentials_store_mode)
    }

    /// Build a CodexAuth for a specific OAuth record.
    pub fn auth_for_record(&self, record_id: &str) -> Option<CodexAuth> {
        let storage = self.auth_storage();
        let store = storage.load().ok().flatten()?;
        let provider = match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
            Some(AuthProviderEntry::Oauth(provider)) => provider,
            _ => return None,
        };
        let record = find_record(provider, record_id)?;
        Some(chatgpt_auth_from_record(storage, record))
    }

    pub(crate) fn oauth_snapshot(&self, namespace: &str) -> std::io::Result<OAuthPoolSnapshot> {
        let storage = self.auth_storage();
        let store = storage.load()?.unwrap_or_default();
        let provider = match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
            Some(AuthProviderEntry::Oauth(provider)) => provider,
            _ => {
                return Ok(OAuthPoolSnapshot {
                    records: Vec::new(),
                    ordered_ids: Vec::new(),
                });
            }
        };
        let namespace = normalize_oauth_namespace(namespace);
        let ordered_ids = record_ids_for_namespace(provider, &namespace);
        let records = provider
            .records
            .iter()
            .filter(|record| record.namespace == namespace)
            .map(|record| OAuthPoolRecord {
                id: record.id.clone(),
                health: record.health.clone(),
            })
            .collect();
        Ok(OAuthPoolSnapshot { records, ordered_ids })
    }

    pub fn oauth_rotation_summary(&self) -> std::io::Result<Option<OAuthRotationSummary>> {
        let storage = self.auth_storage();
        let store = storage.load()?.unwrap_or_default();
        let provider = match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
            Some(AuthProviderEntry::Oauth(provider)) => provider,
            _ => return Ok(None),
        };
        let namespace = normalize_oauth_namespace(DEFAULT_OAUTH_NAMESPACE);
        let records: Vec<_> = provider
            .records
            .iter()
            .filter(|record| record.namespace == namespace)
            .collect();
        if records.len() <= 1 {
            return Ok(None);
        }

        let now = Utc::now();
        let mut summary = OAuthRotationSummary {
            total: records.len(),
            ready: 0,
            cooldown: 0,
            exhausted: 0,
            requires_relogin: 0,
        };

        for record in records {
            let health = &record.health;
            if health.requires_relogin {
                summary.requires_relogin += 1;
            } else if health
                .exhausted_until
                .is_some_and(|until| until > now)
            {
                summary.exhausted += 1;
            } else if health
                .cooldown_until
                .is_some_and(|until| until > now)
            {
                summary.cooldown += 1;
            } else {
                summary.ready += 1;
            }
        }

        Ok(Some(summary))
    }

    pub(crate) fn oauth_move_to_back(&self, namespace: &str, record_id: &str) -> std::io::Result<()> {
        let storage = self.auth_storage();
        let namespace = normalize_oauth_namespace(namespace);
        update_auth_store_best_effort(&self.codex_home, &storage, |store| {
            let provider = match store.providers.get_mut(DEFAULT_OAUTH_PROVIDER_ID) {
                Some(AuthProviderEntry::Oauth(provider)) => provider,
                _ => return Ok(false),
            };
            let mut order = record_ids_for_namespace(provider, &namespace);
            if order.is_empty() {
                return Ok(false);
            }
            if let Some(pos) = order.iter().position(|id| id == record_id) {
                let id = order.remove(pos);
                order.push(id);
            } else {
                return Ok(false);
            }
            provider.order.insert(namespace.clone(), order.clone());
            if let Some(first) = order.first() {
                provider.active.insert(namespace.clone(), first.clone());
            }
            Ok(true)
        })
    }

    fn update_oauth_health_success(
        record: &mut OAuthRecord,
        status_code: u16,
        _now: DateTime<Utc>,
    ) {
        let prev = record.health.clone();
        record.health = OAuthHealth {
            cooldown_until: None,
            exhausted_until: None,
            requires_relogin: false,
            last_status_code: Some(status_code),
            last_error_at: None,
            success_count: prev.success_count + 1,
            failure_count: prev.failure_count,
        };
    }

    fn update_oauth_health_failure(
        record: &mut OAuthRecord,
        status_code: u16,
        cooldown_until: Option<DateTime<Utc>>,
        exhausted_until: Option<DateTime<Utc>>,
        requires_relogin: Option<bool>,
        now: DateTime<Utc>,
    ) {
        let prev = record.health.clone();
        let prev_cooldown = prev.cooldown_until.filter(|until| *until > now);
        let prev_exhausted = prev.exhausted_until.filter(|until| *until > now);
        record.health = OAuthHealth {
            cooldown_until: cooldown_until.or(prev_cooldown),
            exhausted_until: exhausted_until.or(prev_exhausted),
            requires_relogin: requires_relogin.unwrap_or(prev.requires_relogin),
            last_status_code: Some(status_code),
            last_error_at: Some(now),
            success_count: prev.success_count,
            failure_count: prev.failure_count + 1,
        };
    }

    pub(crate) fn oauth_record_outcome(
        &self,
        record_id: &str,
        status_code: u16,
        ok: bool,
        cooldown_until: Option<DateTime<Utc>>,
    ) -> std::io::Result<()> {
        let storage = self.auth_storage();
        update_auth_store_best_effort(&self.codex_home, &storage, |store| {
            let provider = match store.providers.get_mut(DEFAULT_OAUTH_PROVIDER_ID) {
                Some(AuthProviderEntry::Oauth(provider)) => provider,
                _ => return Ok(false),
            };
            let record = match find_record_mut(provider, record_id) {
                Some(record) => record,
                None => return Ok(false),
            };
            let now = Utc::now();
            if ok {
                Self::update_oauth_health_success(record, status_code, now);
            } else {
                Self::update_oauth_health_failure(
                    record,
                    status_code,
                    cooldown_until,
                    None,
                    None,
                    now,
                );
            }
            record.updated_at = now;
            Ok(true)
        })
    }

    pub(crate) fn oauth_record_requires_relogin(
        &self,
        record_id: &str,
        status_code: u16,
    ) -> std::io::Result<()> {
        let storage = self.auth_storage();
        update_auth_store_best_effort(&self.codex_home, &storage, |store| {
            let provider = match store.providers.get_mut(DEFAULT_OAUTH_PROVIDER_ID) {
                Some(AuthProviderEntry::Oauth(provider)) => provider,
                _ => return Ok(false),
            };
            let record = match find_record_mut(provider, record_id) {
                Some(record) => record,
                None => return Ok(false),
            };
            let now = Utc::now();
            Self::update_oauth_health_failure(record, status_code, None, None, Some(true), now);
            record.updated_at = now;
            Ok(true)
        })
    }

    pub(crate) fn oauth_record_exhausted(
        &self,
        record_id: &str,
        status_code: u16,
        exhausted_until: Option<DateTime<Utc>>,
    ) -> std::io::Result<()> {
        let storage = self.auth_storage();
        update_auth_store_best_effort(&self.codex_home, &storage, |store| {
            let provider = match store.providers.get_mut(DEFAULT_OAUTH_PROVIDER_ID) {
                Some(AuthProviderEntry::Oauth(provider)) => provider,
                _ => return Ok(false),
            };
            let record = match find_record_mut(provider, record_id) {
                Some(record) => record,
                None => return Ok(false),
            };
            let now = Utc::now();
            Self::update_oauth_health_failure(
                record,
                status_code,
                None,
                exhausted_until,
                None,
                now,
            );
            record.updated_at = now;
            Ok(true)
        })
    }

    pub async fn refresh_record(&self, record_id: &str) -> Result<(), RefreshTokenError> {
        let storage = self.auth_storage();
        let store = storage
            .load()
            .map_err(RefreshTokenError::Transient)?
            .ok_or_else(|| RefreshTokenError::Transient(std::io::Error::other("Token data is not available.")))?;
        let provider = match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
            Some(AuthProviderEntry::Oauth(provider)) => provider,
            _ => {
                return Err(RefreshTokenError::Transient(std::io::Error::other(
                    "Token data is not available.",
                )))
            }
        };
        let record = find_record(provider, record_id).ok_or_else(|| {
            RefreshTokenError::Transient(std::io::Error::other("Token data is not available."))
        })?;
        let refresh_token = record.tokens.refresh_token.clone();
        let access_token = record.tokens.access_token.clone();
        let client = crate::default_client::create_client();
        let refresh_response = match try_refresh_token(refresh_token.clone(), &client).await {
            Ok(response) => response,
            Err(err) => {
                let refreshed_elsewhere = match storage.load().ok().flatten() {
                    Some(store) => match store.providers.get(DEFAULT_OAUTH_PROVIDER_ID) {
                        Some(AuthProviderEntry::Oauth(provider)) => {
                            find_record(provider, record_id).is_some_and(|latest| {
                                latest.tokens.refresh_token != refresh_token
                                    || latest.tokens.access_token != access_token
                            })
                        }
                        _ => false,
                    },
                    None => false,
                };

                if refreshed_elsewhere {
                    if self
                        .auth_cached()
                        .as_ref()
                        .and_then(|auth| auth.oauth_record_id.as_deref())
                        == Some(record_id)
                    {
                        self.reload();
                    }
                    return Ok(());
                }

                return Err(err);
            }
        };
        update_tokens(
            &self.codex_home,
            &storage,
            Some(record_id),
            refresh_response.id_token,
            refresh_response.access_token,
            refresh_response.refresh_token,
        )
        .await
        .map_err(RefreshTokenError::from)?;

        if self
            .auth_cached()
            .as_ref()
            .and_then(|auth| auth.oauth_record_id.as_deref())
            == Some(record_id)
        {
            self.reload();
        }
        Ok(())
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
            &self.codex_home,
            &auth.storage,
            auth.oauth_record_id.as_deref(),
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
            codex_home.path(),
            &storage,
            None,
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
        let store = storage
            .load()
            .expect("auth.json should parse")
            .expect("auth.json should exist");
        assert_eq!(store.openai_api_key.as_deref(), Some("sk-new"));
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
