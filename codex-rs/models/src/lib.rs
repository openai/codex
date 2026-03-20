mod api_bridge;
mod auth_env_telemetry;
mod error;
mod feedback_tags;
mod response_debug_context;

#[path = "models_manager/cache.rs"]
pub mod cache;
#[path = "models_manager/collaboration_mode_presets.rs"]
pub mod collaboration_mode_presets;
#[path = "models_manager/manager.rs"]
pub mod manager;
#[path = "models_manager/model_info.rs"]
pub mod model_info;
#[path = "models_manager/model_presets.rs"]
pub mod model_presets;
pub mod model_provider_info;

pub use collaboration_mode_presets::CollaborationModesConfig;
pub use collaboration_mode_presets::builtin_collaboration_mode_presets;
pub use manager::ModelsManager;
pub use manager::RefreshStrategy;
pub use model_info::ModelInfoConfigOverrides;
pub use model_info::model_info_from_slug;
pub use model_info::with_config_overrides;
pub use model_provider_info::DEFAULT_LMSTUDIO_PORT;
pub use model_provider_info::DEFAULT_OLLAMA_PORT;
pub use model_provider_info::DEFAULT_WEBSOCKET_CONNECT_TIMEOUT_MS;
pub use model_provider_info::EnvKeyError;
pub use model_provider_info::LEGACY_OLLAMA_CHAT_PROVIDER_ID;
pub use model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::OLLAMA_CHAT_PROVIDER_REMOVED_ERROR;
pub use model_provider_info::OLLAMA_OSS_PROVIDER_ID;
pub use model_provider_info::OPENAI_PROVIDER_ID;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;

/// Convert the client version string to a whole version string (e.g. "1.2.3-alpha.4" -> "1.2.3").
pub fn client_version_to_whole() -> String {
    format!(
        "{}.{}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH")
    )
}
