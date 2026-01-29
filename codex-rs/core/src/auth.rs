pub use codex_auth::AuthCredentialsStoreMode;
pub use codex_auth::AuthDotJson;
pub use codex_auth::AuthManager;
pub use codex_auth::CLIENT_ID;
pub use codex_auth::CODEX_API_KEY_ENV_VAR;
pub use codex_auth::CodexAuth;
pub use codex_auth::LoginRestrictions;
pub use codex_auth::OPENAI_API_KEY_ENV_VAR;
pub use codex_auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
pub use codex_auth::RefreshTokenError;
pub use codex_auth::RefreshTokenFailedError;
pub use codex_auth::RefreshTokenFailedReason;
pub use codex_auth::UnauthorizedRecovery;
pub use codex_auth::load_auth_dot_json;
pub use codex_auth::login_with_api_key;
pub use codex_auth::logout;
pub use codex_auth::read_codex_api_key_from_env;
pub use codex_auth::read_openai_api_key_from_env;
pub use codex_auth::save_auth;

use crate::config::Config;

pub fn enforce_login_restrictions(config: &Config) -> std::io::Result<()> {
    codex_auth::enforce_login_restrictions(&LoginRestrictions {
        codex_home: config.codex_home.clone(),
        forced_login_method: config.forced_login_method,
        forced_chatgpt_workspace_id: config.forced_chatgpt_workspace_id.clone(),
        auth_credentials_store_mode: config.cli_auth_credentials_store_mode,
    })
}
