use super::InstructionSourceGroup;
use super::build_config;
use super::is_non_empty_text_file;
use super::read_json_file;
use crate::RewriteProfile;
use crate::build_mcp_config_from_json_file;
use crate::hook_migration_event_names_cur;
use crate::import_hooks_cur;
use crate::import_subagents_with_rewrite_profile;
use crate::invalid_data_error;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub struct CurSource;

impl CurSource {
    pub const CONFIG_DIR: &'static str = ".cursor";
    pub const MIGRATION_SOURCE: &'static str = "cursor";
    pub const LEGACY_RULES_FILE: &'static str = ".cursorrules";
    pub const HOME_CONFIG_FILE: &'static str = "cli-config.json";
    pub const PROJECT_CONFIG_FILE: &'static str = "cli.json";
    pub const SANDBOX_CONFIG_FILE: &'static str = "sandbox.json";
    pub const HOOKS_CONFIG_FILE: &'static str = "hooks.json";
    pub const SANDBOX_SETTINGS_KEY: &'static str = "__cursorSandbox";
    pub const REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(Self::LEGACY_RULES_FILE, &[])
        .with_case_sensitive_term_variants(&["Cursor"]);

    pub fn effective_settings(
        source_dir: &Path,
        source_settings: &Path,
    ) -> io::Result<Option<JsonValue>> {
        let mut effective = read_json_file(source_settings)?;
        let sandbox_settings = read_json_file(&source_dir.join(Self::SANDBOX_CONFIG_FILE))?;
        if let Some(sandbox_settings) = sandbox_settings {
            let effective =
                effective.get_or_insert_with(|| JsonValue::Object(serde_json::Map::new()));
            let Some(effective) = effective.as_object_mut() else {
                return Err(invalid_data_error(
                    "external agent settings root must be an object",
                ));
            };
            effective.insert(Self::SANDBOX_SETTINGS_KEY.to_string(), sandbox_settings);
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
        let Some(sandbox) = settings
            .get(Self::SANDBOX_SETTINGS_KEY)
            .and_then(JsonValue::as_object)
        else {
            return;
        };
        let sandbox_mode = match sandbox.get("type").and_then(JsonValue::as_str) {
            Some("workspace_readwrite") => Some("workspace-write"),
            Some("read_only") => Some("read-only"),
            _ => None,
        };
        if let Some(sandbox_mode) = sandbox_mode {
            root.insert(
                "sandbox_mode".to_string(),
                TomlValue::String(sandbox_mode.to_string()),
            );
        }
        if sandbox_mode != Some("workspace-write") {
            return;
        }

        let mut workspace_write = toml::map::Map::new();
        if let Some(paths) = sandbox
            .get("additionalReadwritePaths")
            .and_then(JsonValue::as_array)
        {
            let paths = paths
                .iter()
                .filter_map(JsonValue::as_str)
                .filter(|path| Path::new(path).is_absolute())
                .map(|path| TomlValue::String(path.to_string()))
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                workspace_write.insert("writable_roots".to_string(), TomlValue::Array(paths));
            }
        }
        if sandbox.get("disableTmpWrite").and_then(JsonValue::as_bool) == Some(true) {
            workspace_write.insert("exclude_slash_tmp".to_string(), TomlValue::Boolean(true));
            workspace_write.insert(
                "exclude_tmpdir_env_var".to_string(),
                TomlValue::Boolean(true),
            );
        }
        if sandbox
            .get("networkPolicy")
            .and_then(JsonValue::as_object)
            .and_then(|network| network.get("default"))
            .and_then(JsonValue::as_str)
            == Some("allow")
        {
            workspace_write.insert("network_access".to_string(), TomlValue::Boolean(true));
        }
        if !workspace_write.is_empty() {
            root.insert(
                "sandbox_workspace_write".to_string(),
                TomlValue::Table(workspace_write),
            );
        }
    }

    pub fn build_mcp_config(source_dir: &Path) -> io::Result<TomlValue> {
        build_mcp_config_from_json_file(&source_dir.join("mcp.json"))
    }

    pub fn repo_instruction_source_groups(
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        let source = repo_root.join(Self::LEGACY_RULES_FILE);
        Ok(is_non_empty_text_file(&source)?
            .then(|| InstructionSourceGroup {
                scope: repo_root.to_path_buf(),
                sources: vec![source],
            })
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
        hook_migration_event_names_cur(
            source_dir,
            &source_dir.join(Self::HOOKS_CONFIG_FILE),
            target_hooks,
            Self::REWRITE_PROFILE,
        )
    }

    pub fn import_hooks(source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
        import_hooks_cur(
            source_dir,
            &source_dir.join(Self::HOOKS_CONFIG_FILE),
            target_hooks,
            Self::REWRITE_PROFILE,
        )
    }
}
