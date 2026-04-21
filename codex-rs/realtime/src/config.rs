use codex_config::config_toml::RealtimeAudioConfig;
use codex_config::config_toml::RealtimeConfig;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RealtimeFeaturesConfig {
    pub audio: RealtimeAudioConfig,
    pub session: RealtimeConfig,
    pub websocket_base_url: Option<String>,
    pub websocket_model: Option<String>,
    pub websocket_backend_prompt: Option<String>,
    pub websocket_startup_context: Option<String>,
    pub start_instructions: Option<String>,
}
