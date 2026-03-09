mod custom_ca;
mod device_code_auth;
mod pkce;
// Hidden because this exists only to let the spawned `login_ca_probe` binary call the
// probe-specific client builder without exposing that workaround as part of the normal API.
// `login_ca_probe` is a separate binary target, not a `#[cfg(test)]` module inside this crate, so
// it cannot call crate-private helpers and would not see test-only modules.
#[doc(hidden)]
pub mod probe_support;
mod server;

pub use custom_ca::BuildLoginHttpClientError;
pub use custom_ca::build_login_http_client;
pub use device_code_auth::DeviceCode;
pub use device_code_auth::complete_device_code_login;
pub use device_code_auth::request_device_code;
pub use device_code_auth::run_device_code_login;
pub use server::LoginServer;
pub use server::ServerOptions;
pub use server::ShutdownHandle;
pub use server::build_login_http_client;
pub use server::run_login_server;

// Re-export commonly used auth types and helpers from codex-core for compatibility
pub use codex_app_server_protocol::AuthMode;
pub use codex_core::AuthManager;
pub use codex_core::CodexAuth;
pub use codex_core::auth::AuthDotJson;
pub use codex_core::auth::CLIENT_ID;
pub use codex_core::auth::CODEX_API_KEY_ENV_VAR;
pub use codex_core::auth::OPENAI_API_KEY_ENV_VAR;
pub use codex_core::auth::login_with_api_key;
pub use codex_core::auth::logout;
pub use codex_core::auth::save_auth;
pub use codex_core::token_data::TokenData;
