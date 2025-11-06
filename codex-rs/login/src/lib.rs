mod device_code_auth;
mod pkce;
mod server;

// Re-export commonly used auth types and helpers from codex-core for compatibility
pub use codex_app_server_protocol::AuthMode;
pub use codex_core::auth::{
    AuthDotJson, CLIENT_ID, CODEX_API_KEY_ENV_VAR, OPENAI_API_KEY_ENV_VAR, login_with_api_key,
    logout, save_auth,
};
pub use codex_core::token_data::TokenData;
pub use codex_core::{AuthManager, CodexAuth};
pub use device_code_auth::run_device_code_login;
pub use server::{LoginServer, ServerOptions, ShutdownHandle, run_login_server};
