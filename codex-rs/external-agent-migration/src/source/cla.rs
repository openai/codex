use super::InstructionSourceGroup;
use super::build_config;
use super::is_non_empty_text_file;
use super::read_json_file;
use crate::RewriteProfile;
use crate::build_mcp_config_from_external;
use crate::hook_migration_event_names_cla;
use crate::import_hooks_cla;
use crate::import_subagents_with_rewrite_profile;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

pub struct ClaSource;

impl ClaSource {
    pub const CONFIG_DIR: &'static str = ".claude";
    pub const CONFIG_MD: &'static str = "CLAUDE.md";
    pub const SETTINGS_FILE: &'static str = "settings.json";
    pub const REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(
        Self::CONFIG_MD,
        &[
            "claude code",
            "claude-code",
            "claude_code",
            "claudecode",
            "claude",
        ],
    );

    pub fn connector_metadata_roots(external_agent_home: &Path) -> Vec<PathBuf> {
        let Some(home) = external_agent_home
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        else {
            return Vec::new();
        };

        #[cfg(target_os = "macos")]
        {
            vec![home.join("Library/Application Support/Claude")]
        }

        #[cfg(target_os = "windows")]
        {
            let default_roaming = home.join("AppData/Roaming");
            let default_local = home.join("AppData/Local");
            let roaming = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| default_roaming.clone());
            let local = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| default_local.clone());
            let mut roots = vec![
                local.join("Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude"),
                roaming.join("Claude"),
                default_local.join("Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude"),
                default_roaming.join("Claude"),
            ];
            roots.sort();
            roots.dedup();
            roots
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            vec![home.join(".config/Claude")]
        }
    }

    pub fn effective_settings(project_settings: &Path) -> io::Result<Option<JsonValue>> {
        let mut effective = read_json_file(project_settings)?;
        let Some(settings_dir) = project_settings.parent() else {
            return Ok(effective);
        };
        let local_settings = match read_json_file(&settings_dir.join("settings.local.json")) {
            Ok(Some(local_settings)) => local_settings,
            Ok(None) => return Ok(effective),
            Err(err) if err.kind() == io::ErrorKind::InvalidData => return Ok(effective),
            Err(err) => return Err(err),
        };
        if let Some(effective) = effective.as_mut() {
            merge_json_settings(effective, &local_settings);
        } else {
            effective = Some(local_settings);
        }
        Ok(effective)
    }

    pub fn build_config(settings: &JsonValue) -> io::Result<TomlValue> {
        build_config(settings, Self::append_config)
    }

    pub fn append_config(
        root: &mut toml::map::Map<String, TomlValue>,
        settings: &serde_json::Map<String, JsonValue>,
    ) {
        if settings
            .get("sandbox")
            .and_then(JsonValue::as_object)
            .and_then(|sandbox| sandbox.get("enabled"))
            .and_then(JsonValue::as_bool)
            == Some(true)
        {
            root.insert(
                "sandbox_mode".to_string(),
                TomlValue::String("workspace-write".to_string()),
            );
        }
    }

    pub fn build_mcp_config(
        source_root: &Path,
        external_agent_home: &Path,
        settings: Option<&JsonValue>,
    ) -> io::Result<TomlValue> {
        build_mcp_config_from_external(source_root, Some(external_agent_home), settings)
    }

    pub fn repo_instruction_source_groups(
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        for candidate in [
            repo_root.join(Self::CONFIG_MD),
            repo_root.join(Self::CONFIG_DIR).join(Self::CONFIG_MD),
        ] {
            if is_non_empty_text_file(&candidate)? {
                return Ok(vec![InstructionSourceGroup {
                    scope: repo_root.to_path_buf(),
                    sources: vec![candidate],
                }]);
            }
        }
        Ok(Vec::new())
    }

    pub fn home_instruction_sources(external_agent_home: &Path) -> io::Result<Vec<PathBuf>> {
        let path = external_agent_home.join(Self::CONFIG_MD);
        Ok(is_non_empty_text_file(&path)?
            .then_some(path)
            .into_iter()
            .collect())
    }

    pub fn read_instruction_source(path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    pub fn import_subagents(source_agents: &Path, target_agents: &Path) -> io::Result<Vec<String>> {
        import_subagents_with_rewrite_profile(source_agents, target_agents, Self::REWRITE_PROFILE)
    }

    pub fn hook_event_names(source_dir: &Path, target_hooks: &Path) -> io::Result<Vec<String>> {
        hook_migration_event_names_cla(source_dir, target_hooks, Self::REWRITE_PROFILE)
    }

    pub fn import_hooks(source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
        import_hooks_cla(source_dir, target_hooks, Self::REWRITE_PROFILE)
    }
}

fn merge_json_settings(existing: &mut JsonValue, incoming: &JsonValue) {
    match (existing, incoming) {
        (JsonValue::Object(existing), JsonValue::Object(incoming)) => {
            for (key, incoming_value) in incoming {
                match existing.get_mut(key) {
                    Some(existing_value) => merge_json_settings(existing_value, incoming_value),
                    None => {
                        existing.insert(key.clone(), incoming_value.clone());
                    }
                }
            }
        }
        (existing, incoming) => *existing = incoming.clone(),
    }
}
