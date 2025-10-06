mod device_code_auth;
mod pkce;
mod server;

pub use device_code_auth::run_device_code_login;
pub use server::LoginServer;
pub use server::ServerOptions;
pub use server::ShutdownHandle;
pub use server::run_login_server;

// Re-export commonly used auth types and helpers from codex-core for compatibility
pub use codex_app_server_protocol::AuthMode;
pub use codex_core::AuthManager;
pub use codex_core::CodexAuth;
pub use codex_core::auth::AuthDotJson;
pub use codex_core::auth::CLIENT_ID;
pub use codex_core::auth::CODEX_API_KEY_ENV_VAR;
pub use codex_core::auth::OPENAI_API_KEY_ENV_VAR;
pub use codex_core::auth::get_auth_file;
pub use codex_core::auth::login_with_api_key;
pub use codex_core::auth::logout;
pub use codex_core::auth::try_read_auth_json;
pub use codex_core::auth::write_auth_json;
pub use codex_core::token_data::TokenData;
