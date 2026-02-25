mod device_code_auth;
mod pkce;
mod server;

pub use device_code_auth::DeviceCode;
pub use device_code_auth::complete_device_code_login;
pub use device_code_auth::request_device_code;
pub use device_code_auth::run_device_code_login;
pub use server::LoginServer;
pub use server::ServerOptions;
pub use server::ShutdownHandle;
pub use server::run_login_server;

// Re-export commonly used auth types and helpers from helios-core for compatibility
pub use helios_app_server_protocol::AuthMode;
pub use helios_core::AuthManager;
pub use helios_core::CodexAuth;
pub use helios_core::auth::AuthDotJson;
pub use helios_core::auth::CLIENT_ID;
pub use helios_core::auth::HELIOS_API_KEY_ENV_VAR;
pub use helios_core::auth::OPENAI_API_KEY_ENV_VAR;
pub use helios_core::auth::login_with_api_key;
pub use helios_core::auth::logout;
pub use helios_core::auth::save_auth;
pub use helios_core::token_data::TokenData;
