#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
#[cfg(not(target_arch = "wasm32"))]
pub mod token_data;

#[cfg(not(target_arch = "wasm32"))]
mod device_code_auth;
#[cfg(not(target_arch = "wasm32"))]
mod pkce;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use codex_client::BuildCustomCaTransportError as BuildLoginHttpClientError;
#[cfg(not(target_arch = "wasm32"))]
pub use device_code_auth::DeviceCode;
#[cfg(not(target_arch = "wasm32"))]
pub use device_code_auth::complete_device_code_login;
#[cfg(not(target_arch = "wasm32"))]
pub use device_code_auth::request_device_code;
#[cfg(not(target_arch = "wasm32"))]
pub use device_code_auth::run_device_code_login;
#[cfg(not(target_arch = "wasm32"))]
pub use server::LoginServer;
#[cfg(not(target_arch = "wasm32"))]
pub use server::ServerOptions;
#[cfg(not(target_arch = "wasm32"))]
pub use server::ShutdownHandle;
#[cfg(not(target_arch = "wasm32"))]
pub use server::run_login_server;

#[cfg(not(target_arch = "wasm32"))]
pub use auth::AuthConfig;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::AuthCredentialsStoreMode;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::AuthDotJson;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::AuthManager;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::CLIENT_ID;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::CODEX_API_KEY_ENV_VAR;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::CodexAuth;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::ExternalAuthChatgptMetadata;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::ExternalAuthRefreshContext;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::ExternalAuthRefreshReason;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::ExternalAuthRefresher;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::ExternalAuthTokens;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::OPENAI_API_KEY_ENV_VAR;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::RefreshTokenError;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::UnauthorizedRecovery;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::default_client;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::enforce_login_restrictions;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::load_auth_dot_json;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::login_with_api_key;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::logout;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::read_openai_api_key_from_env;
#[cfg(not(target_arch = "wasm32"))]
pub use auth::save_auth;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_app_server_protocol::AuthMode;
#[cfg(not(target_arch = "wasm32"))]
pub use token_data::TokenData;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
