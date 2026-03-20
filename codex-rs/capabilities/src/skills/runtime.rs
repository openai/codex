use codex_config::ConfigLayerStack;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SkillLoadRequest {
    pub cwd: PathBuf,
    pub config_layer_stack: ConfigLayerStack,
    pub bundled_skills_enabled: bool,
    pub plugin_skill_roots: Vec<PathBuf>,
    pub extra_user_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RemoteSkillRequest {
    pub chatgpt_base_url: String,
    pub codex_home: PathBuf,
}
