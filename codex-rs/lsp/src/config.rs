use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::registry::LanguageServerId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LspMode {
    Off,
    Auto,
    On,
}

impl Default for LspMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LspDiagnosticsInPrompt {
    Off,
    Errors,
    ErrorsAndWarnings,
}

impl Default for LspDiagnosticsInPrompt {
    fn default() -> Self {
        Self::Errors
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct LspServerConfigToml {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct LspConfigToml {
    #[serde(default)]
    pub mode: Option<LspMode>,
    #[serde(default)]
    pub auto_install: Option<bool>,
    #[serde(default)]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub diagnostics_in_prompt: Option<LspDiagnosticsInPrompt>,
    #[serde(default)]
    pub max_diagnostics_per_file: Option<usize>,
    #[serde(default)]
    pub max_files: Option<usize>,
    #[serde(default)]
    pub servers: Option<HashMap<String, LspServerConfigToml>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    pub enabled: bool,
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspConfig {
    pub mode: LspMode,
    pub auto_install: bool,
    pub install_dir: PathBuf,
    pub diagnostics_in_prompt: LspDiagnosticsInPrompt,
    pub max_diagnostics_per_file: usize,
    pub max_files: usize,
    pub servers: HashMap<LanguageServerId, LspServerConfig>,
}

impl LspConfig {
    pub fn resolve(toml: Option<&LspConfigToml>, codex_home: &Path) -> Self {
        let mode = toml.and_then(|cfg| cfg.mode).unwrap_or_default();
        let auto_install = toml.and_then(|cfg| cfg.auto_install).unwrap_or(false);
        let diagnostics_in_prompt = toml
            .and_then(|cfg| cfg.diagnostics_in_prompt)
            .unwrap_or_default();
        let max_diagnostics_per_file = toml
            .and_then(|cfg| cfg.max_diagnostics_per_file)
            .unwrap_or(10);
        let max_files = toml.and_then(|cfg| cfg.max_files).unwrap_or(5);
        let install_dir = toml
            .and_then(|cfg| cfg.install_dir.as_deref())
            .map(|raw| resolve_install_dir(raw, codex_home))
            .unwrap_or_else(|| codex_home.join("lsp"));

        let mut servers = HashMap::new();
        let overrides = toml.and_then(|cfg| cfg.servers.as_ref());
        for id in LanguageServerId::all() {
            let override_cfg = overrides.and_then(|map| map.get(id.as_str()));
            servers.insert(
                id,
                LspServerConfig {
                    enabled: override_cfg.and_then(|cfg| cfg.enabled).unwrap_or(true),
                    command: override_cfg.and_then(|cfg| cfg.command.clone()),
                },
            );
        }

        Self {
            mode,
            auto_install,
            install_dir,
            diagnostics_in_prompt,
            max_diagnostics_per_file,
            max_files,
            servers,
        }
    }

    pub fn server_config(&self, id: LanguageServerId) -> LspServerConfig {
        self.servers.get(&id).cloned().unwrap_or(LspServerConfig {
            enabled: true,
            command: None,
        })
    }
}

fn resolve_install_dir(raw: &str, codex_home: &Path) -> PathBuf {
    let expanded = raw.replace("$CODEX_HOME", &codex_home.to_string_lossy());
    if let Some(rest) = expanded.strip_prefix("~")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest.trim_start_matches('/'));
    }
    PathBuf::from(expanded)
}
