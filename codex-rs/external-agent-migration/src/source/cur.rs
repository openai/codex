use super::InstructionSourceGroup;
use super::build_config;
use super::is_non_empty_text_file;
use super::read_json_file;
use crate::RewriteProfile;
use crate::build_mcp_config_from_json_file;
use crate::hook_migration_event_names_cur;
use crate::import_hooks_cur;
use crate::import_subagents_with_rewrite_profile;
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
    pub const HOOKS_CONFIG_FILE: &'static str = "hooks.json";
    pub const REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(Self::LEGACY_RULES_FILE, &[])
        .with_case_sensitive_term_variants(&["Cursor"]);

    pub fn effective_settings(source_settings: &Path) -> io::Result<Option<JsonValue>> {
        read_json_file(source_settings)
    }

    pub fn build_config(settings: &JsonValue) -> io::Result<TomlValue> {
        build_config(settings, |_, _| {})
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
