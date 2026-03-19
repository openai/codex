pub mod auth;
pub mod token_data;

mod device_code_auth;
mod pkce;
mod server;

pub use codex_client::BuildCustomCaTransportError as BuildLoginHttpClientError;
pub use device_code_auth::DeviceCode;
pub use device_code_auth::complete_device_code_login;
pub use device_code_auth::request_device_code;
pub use device_code_auth::run_device_code_login;
pub use server::LoginServer;
pub use server::ServerOptions;
pub use server::ShutdownHandle;
pub use server::run_login_server;

pub use auth::AuthCredentialsStoreMode;
pub use auth::AuthDotJson;
pub use auth::AuthManager;
pub use auth::CLIENT_ID;
pub use auth::CODEX_API_KEY_ENV_VAR;
pub use auth::CodexAuth;
pub use auth::OPENAI_API_KEY_ENV_VAR;
pub use auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
pub use auth::RefreshTokenError;
pub use auth::UnauthorizedRecovery;
pub use auth::login_with_api_key;
pub use auth::logout;
pub use auth::read_openai_api_key_from_env;
pub use auth::save_auth;
pub use codex_app_server_protocol::AuthMode;
pub use token_data::TokenData;
