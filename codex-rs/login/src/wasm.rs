use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
pub use codex_app_server_protocol::AuthMode;
use codex_client::BuildCustomCaTransportError;
use codex_client::CodexHttpClient;
use codex_protocol::config_types::ModelProviderAuthInfo;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use thiserror::Error;

pub mod token_data {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
    pub struct TokenData {
        pub id_token: IdTokenInfo,
        pub access_token: String,
        pub refresh_token: String,
        pub account_id: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    pub struct IdTokenInfo {
        pub email: Option<String>,
        pub chatgpt_plan_type: Option<PlanType>,
        pub chatgpt_user_id: Option<String>,
        pub chatgpt_account_id: Option<String>,
        pub raw_jwt: String,
    }

    impl IdTokenInfo {
        pub fn is_workspace_account(&self) -> bool {
            matches!(
                self.chatgpt_plan_type,
                Some(PlanType::Known(plan)) if plan.is_workspace_account()
            )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum PlanType {
        Known(KnownPlan),
        Unknown(String),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum KnownPlan {
        Free,
        Go,
        Plus,
        Pro,
        Team,
        #[serde(rename = "self_serve_business_usage_based")]
        SelfServeBusinessUsageBased,
        Business,
        #[serde(rename = "enterprise_cbp_usage_based")]
        EnterpriseCbpUsageBased,
        #[serde(alias = "hc")]
        Enterprise,
        Edu,
    }

    impl KnownPlan {
        pub fn is_workspace_account(self) -> bool {
            matches!(
                self,
                Self::Team
                    | Self::SelfServeBusinessUsageBased
                    | Self::Business
                    | Self::EnterpriseCbpUsageBased
                    | Self::Enterprise
                    | Self::Edu
            )
        }
    }
}

pub mod auth {
    use super::*;

    pub const CLIENT_ID: &str = "codex-wasm";
    pub const CODEX_API_KEY_ENV_VAR: &str = "CODEX_API_KEY";
    pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
    pub const REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_REFRESH_TOKEN_URL_OVERRIDE";

    #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum AuthCredentialsStoreMode {
        #[default]
        File,
        Keyring,
        Auto,
        Ephemeral,
    }

    #[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
    pub struct AuthDotJson {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub auth_mode: Option<AuthMode>,
        #[serde(rename = "OPENAI_API_KEY")]
        pub openai_api_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tokens: Option<super::token_data::TokenData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_refresh: Option<DateTime<Utc>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Error)]
    #[error("{message}")]
    pub struct RefreshTokenFailedError {
        pub reason: RefreshTokenFailedReason,
        pub message: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RefreshTokenFailedReason {
        Expired,
        Exhausted,
        Revoked,
        Other,
    }

    #[derive(Debug, Error)]
    pub enum RefreshTokenError {
        #[error("{0}")]
        Permanent(#[from] RefreshTokenFailedError),
        #[error(transparent)]
        Transient(#[from] std::io::Error),
    }

    #[derive(Debug, Clone)]
    pub enum CodexAuth {
        ApiKey(ApiKeyAuth),
        Chatgpt(ChatgptAuth),
        ChatgptAuthTokens(ChatgptAuthTokens),
    }

    #[derive(Debug, Clone)]
    pub struct ApiKeyAuth {
        api_key: String,
    }

    #[derive(Debug, Clone, Default)]
    pub struct ChatgptAuth {
        pub token_data: Option<super::token_data::TokenData>,
    }

    #[derive(Debug, Clone, Default)]
    pub struct ChatgptAuthTokens {
        pub token_data: Option<super::token_data::TokenData>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ExternalAuthTokens {
        pub access_token: String,
        pub chatgpt_metadata: Option<ExternalAuthChatgptMetadata>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ExternalAuthChatgptMetadata {
        pub account_id: String,
        pub plan_type: Option<String>,
    }

    impl ExternalAuthTokens {
        pub fn access_token_only(access_token: impl Into<String>) -> Self {
            Self {
                access_token: access_token.into(),
                chatgpt_metadata: None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ExternalAuthRefreshReason {
        Unauthorized,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ExternalAuthRefreshContext {
        pub reason: ExternalAuthRefreshReason,
        pub previous_account_id: Option<String>,
    }

    #[async_trait]
    pub trait ExternalAuthRefresher: Send + Sync {
        async fn resolve(&self) -> std::io::Result<Option<ExternalAuthTokens>> {
            Ok(None)
        }

        async fn refresh(
            &self,
            _context: ExternalAuthRefreshContext,
        ) -> std::io::Result<ExternalAuthTokens>;
    }

    #[derive(Debug)]
    pub struct UnauthorizedRecovery {
        done: bool,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct UnauthorizedRecoveryStepResult {
        auth_state_changed: Option<bool>,
    }

    impl UnauthorizedRecoveryStepResult {
        pub fn auth_state_changed(&self) -> Option<bool> {
            self.auth_state_changed
        }
    }

    impl UnauthorizedRecovery {
        pub fn has_next(&self) -> bool {
            !self.done
        }

        pub fn mode_name(&self) -> &'static str {
            "managed"
        }

        pub fn step_name(&self) -> &'static str {
            "done"
        }

        pub fn unavailable_reason(&self) -> &'static str {
            if self.done {
                "recovery_exhausted"
            } else {
                "ready"
            }
        }

        pub async fn next(&mut self) -> Result<UnauthorizedRecoveryStepResult, RefreshTokenError> {
            self.done = true;
            Ok(UnauthorizedRecoveryStepResult {
                auth_state_changed: Some(false),
            })
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct AuthConfig;

    #[derive(Debug)]
    pub struct AuthManager {
        auth: RwLock<Option<CodexAuth>>,
        enable_codex_api_key_env: bool,
    }

    impl PartialEq for CodexAuth {
        fn eq(&self, other: &Self) -> bool {
            self.api_auth_mode() == other.api_auth_mode()
        }
    }

    impl CodexAuth {
        pub fn from_api_key(api_key: &str) -> Self {
            Self::ApiKey(ApiKeyAuth {
                api_key: api_key.to_string(),
            })
        }

        pub fn create_dummy_chatgpt_auth_for_testing() -> Self {
            Self::Chatgpt(ChatgptAuth::default())
        }

        pub fn auth_mode(&self) -> AuthMode {
            match self {
                Self::ApiKey(_) => AuthMode::ApiKey,
                Self::Chatgpt(_) | Self::ChatgptAuthTokens(_) => AuthMode::Chatgpt,
            }
        }

        pub fn api_auth_mode(&self) -> AuthMode {
            match self {
                Self::ApiKey(_) => AuthMode::ApiKey,
                Self::Chatgpt(_) => AuthMode::Chatgpt,
                Self::ChatgptAuthTokens(_) => AuthMode::ChatgptAuthTokens,
            }
        }

        pub fn is_api_key_auth(&self) -> bool {
            matches!(self, Self::ApiKey(_))
        }

        pub fn is_chatgpt_auth(&self) -> bool {
            matches!(self, Self::Chatgpt(_))
        }

        pub fn is_external_chatgpt_tokens(&self) -> bool {
            matches!(self, Self::ChatgptAuthTokens(_))
        }

        pub fn api_key(&self) -> Option<&str> {
            match self {
                Self::ApiKey(auth) => Some(auth.api_key.as_str()),
                Self::Chatgpt(_) | Self::ChatgptAuthTokens(_) => None,
            }
        }

        pub fn get_token_data(&self) -> Result<super::token_data::TokenData, std::io::Error> {
            match self {
                Self::ApiKey(_) => Err(std::io::Error::other("Token data is not available.")),
                Self::Chatgpt(auth) => auth
                    .token_data
                    .clone()
                    .ok_or_else(|| std::io::Error::other("Token data is not available.")),
                Self::ChatgptAuthTokens(auth) => auth
                    .token_data
                    .clone()
                    .ok_or_else(|| std::io::Error::other("Token data is not available.")),
            }
        }

        pub fn get_token(&self) -> Result<String, std::io::Error> {
            match self {
                Self::ApiKey(auth) => Ok(auth.api_key.clone()),
                Self::Chatgpt(_) | Self::ChatgptAuthTokens(_) => {
                    Ok(self.get_token_data()?.access_token)
                }
            }
        }

        pub fn get_account_id(&self) -> Option<String> {
            self.get_current_token_data()
                .and_then(|token| token.account_id)
        }

        pub fn get_account_email(&self) -> Option<String> {
            self.get_current_token_data()
                .and_then(|token| token.id_token.email)
        }

        pub fn get_current_auth_json(&self) -> Option<AuthDotJson> {
            match self {
                Self::ApiKey(auth) => Some(AuthDotJson {
                    auth_mode: Some(AuthMode::ApiKey),
                    openai_api_key: Some(auth.api_key.clone()),
                    tokens: None,
                    last_refresh: None,
                }),
                Self::Chatgpt(auth) => auth.token_data.clone().map(|tokens| AuthDotJson {
                    auth_mode: Some(AuthMode::Chatgpt),
                    openai_api_key: None,
                    tokens: Some(tokens),
                    last_refresh: None,
                }),
                Self::ChatgptAuthTokens(auth) => {
                    auth.token_data.clone().map(|tokens| AuthDotJson {
                        auth_mode: Some(AuthMode::ChatgptAuthTokens),
                        openai_api_key: None,
                        tokens: Some(tokens),
                        last_refresh: None,
                    })
                }
            }
        }

        fn get_current_token_data(&self) -> Option<super::token_data::TokenData> {
            match self {
                Self::ApiKey(_) => None,
                Self::Chatgpt(auth) => auth.token_data.clone(),
                Self::ChatgptAuthTokens(auth) => auth.token_data.clone(),
            }
        }
    }

    impl AuthManager {
        pub fn new(
            _codex_home: PathBuf,
            enable_codex_api_key_env: bool,
            _auth_credentials_store_mode: AuthCredentialsStoreMode,
        ) -> Self {
            let auth =
                read_openai_api_key_from_env().map(|api_key| CodexAuth::from_api_key(&api_key));
            Self {
                auth: RwLock::new(auth),
                enable_codex_api_key_env,
            }
        }

        pub fn from_auth_for_testing(auth: CodexAuth) -> Arc<Self> {
            Arc::new(Self {
                auth: RwLock::new(Some(auth)),
                enable_codex_api_key_env: false,
            })
        }

        pub fn from_auth_for_testing_with_home(auth: CodexAuth, _codex_home: PathBuf) -> Arc<Self> {
            Self::from_auth_for_testing(auth)
        }

        pub fn external_bearer_only(_config: ModelProviderAuthInfo) -> Arc<Self> {
            Arc::new(Self {
                auth: RwLock::new(None),
                enable_codex_api_key_env: false,
            })
        }

        pub fn auth_cached(&self) -> Option<CodexAuth> {
            self.auth.read().ok().and_then(|auth| auth.clone())
        }

        pub async fn auth(&self) -> Option<CodexAuth> {
            self.auth_cached()
        }

        pub fn auth_mode(&self) -> Option<AuthMode> {
            self.auth_cached().map(|auth| auth.auth_mode())
        }

        pub fn codex_api_key_env_enabled(&self) -> bool {
            self.enable_codex_api_key_env
        }

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
            UnauthorizedRecovery { done: false }
        }
    }

    pub mod default_client {
        use super::*;
        use reqwest::header::HeaderMap;
        use reqwest::header::HeaderValue;

        pub const DEFAULT_ORIGINATOR: &str = "codex_wasm";
        pub const CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR: &str =
            "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
        pub const RESIDENCY_HEADER_NAME: &str = "x-openai-internal-codex-residency";

        pub use codex_config::ResidencyRequirement;

        #[derive(Debug, Clone)]
        pub struct Originator {
            pub value: String,
            pub header_value: HeaderValue,
        }

        pub fn originator() -> Originator {
            Originator {
                value: DEFAULT_ORIGINATOR.to_string(),
                header_value: HeaderValue::from_static(DEFAULT_ORIGINATOR),
            }
        }

        pub fn is_first_party_originator(originator_value: &str) -> bool {
            originator_value == DEFAULT_ORIGINATOR
        }

        pub fn is_first_party_chat_originator(originator_value: &str) -> bool {
            originator_value == "codex_chatgpt_desktop"
        }

        pub fn set_default_originator(_value: String) -> Result<(), super::SetOriginatorError> {
            Ok(())
        }

        pub fn set_default_client_residency_requirement(
            _enforce_residency: Option<ResidencyRequirement>,
        ) {
        }

        pub fn get_codex_user_agent() -> String {
            format!("{DEFAULT_ORIGINATOR}/{}", env!("CARGO_PKG_VERSION"))
        }

        pub fn create_client() -> CodexHttpClient {
            CodexHttpClient::new(build_reqwest_client())
        }

        pub fn build_reqwest_client() -> reqwest::Client {
            try_build_reqwest_client().unwrap_or_else(|_| reqwest::Client::new())
        }

        pub fn try_build_reqwest_client() -> Result<reqwest::Client, BuildCustomCaTransportError> {
            let headers = default_headers();
            codex_client::build_reqwest_client_with_custom_ca(
                reqwest::Client::builder()
                    .user_agent(get_codex_user_agent())
                    .default_headers(headers),
            )
        }

        pub fn default_headers() -> HeaderMap {
            let mut headers = HeaderMap::new();
            headers.insert("originator", originator().header_value);
            headers
        }

        pub use codex_client::CodexRequestBuilder;
    }

    #[derive(Debug)]
    pub enum SetOriginatorError {
        InvalidHeaderValue,
        AlreadyInitialized,
    }

    pub fn read_openai_api_key_from_env() -> Option<String> {
        std::env::var(OPENAI_API_KEY_ENV_VAR).ok()
    }

    pub fn load_auth_dot_json(_codex_home: &Path) -> std::io::Result<Option<AuthDotJson>> {
        Ok(None)
    }

    pub fn save_auth(_codex_home: &Path, _auth_dot_json: &AuthDotJson) -> std::io::Result<()> {
        Ok(())
    }

    pub fn logout(
        _codex_home: &Path,
        _auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> std::io::Result<()> {
        Ok(())
    }

    pub async fn login_with_api_key(
        codex_home: PathBuf,
        api_key: String,
        _auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> std::io::Result<AuthDotJson> {
        let auth = AuthDotJson {
            auth_mode: Some(AuthMode::ApiKey),
            openai_api_key: Some(api_key),
            tokens: None,
            last_refresh: None,
        };
        save_auth(&codex_home, &auth)?;
        Ok(auth)
    }

    pub fn enforce_login_restrictions(
        _forced_login_method: Option<codex_protocol::config_types::ForcedLoginMethod>,
        _auth_mode: Option<AuthMode>,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

pub use auth::AuthConfig;
pub use auth::AuthCredentialsStoreMode;
pub use auth::AuthDotJson;
pub use auth::AuthManager;
pub use auth::CLIENT_ID;
pub use auth::CODEX_API_KEY_ENV_VAR;
pub use auth::CodexAuth;
pub use auth::ExternalAuthChatgptMetadata;
pub use auth::ExternalAuthRefreshContext;
pub use auth::ExternalAuthRefreshReason;
pub use auth::ExternalAuthRefresher;
pub use auth::ExternalAuthTokens;
pub use auth::OPENAI_API_KEY_ENV_VAR;
pub use auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
pub use auth::RefreshTokenError;
pub use auth::UnauthorizedRecovery;
pub use auth::default_client;
pub use auth::enforce_login_restrictions;
pub use auth::load_auth_dot_json;
pub use auth::login_with_api_key;
pub use auth::logout;
pub use auth::read_openai_api_key_from_env;
pub use auth::save_auth;
pub use token_data::TokenData;

pub type BuildLoginHttpClientError = BuildCustomCaTransportError;

#[derive(Debug, Clone)]
pub struct DeviceCode;

#[derive(Debug, Clone)]
pub struct LoginServer;

#[derive(Debug, Clone)]
pub struct ServerOptions;

#[derive(Debug, Clone)]
pub struct ShutdownHandle;

pub async fn complete_device_code_login(_device_code: DeviceCode) -> std::io::Result<AuthDotJson> {
    Err(std::io::Error::other(
        "device code login is unavailable on wasm32",
    ))
}

pub async fn request_device_code() -> std::io::Result<DeviceCode> {
    Err(std::io::Error::other(
        "device code login is unavailable on wasm32",
    ))
}

pub async fn run_device_code_login() -> std::io::Result<AuthDotJson> {
    Err(std::io::Error::other(
        "device code login is unavailable on wasm32",
    ))
}

pub async fn run_login_server(_options: ServerOptions) -> std::io::Result<ShutdownHandle> {
    Err(std::io::Error::other(
        "login server is unavailable on wasm32",
    ))
}
