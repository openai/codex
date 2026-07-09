use codex_external_agent_migration::CommandDescriptionMode;
use codex_external_agent_migration::CommandMigrationProfile;
use codex_external_agent_migration::RewriteProfile;
use codex_external_agent_migration::build_mcp_config_from_external;
use codex_external_agent_migration::count_missing_commands_with_profile;
use codex_external_agent_migration::hook_migration_event_names;
use codex_external_agent_migration::import_commands_with_profile;
use codex_external_agent_migration::import_hooks;
use codex_external_agent_migration::import_subagents_with_rewrite_profile;
use codex_external_agent_migration::missing_command_names_with_profile;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use super::is_non_empty_text_file;
use super::source::InstructionSourceGroup;

pub(super) const CONFIG_DIR: &str = ".claude";
pub(super) const CONFIG_MD: &str = "CLAUDE.md";
pub(super) const KNOWN_MARKETPLACES_PATH: &str = "plugins/known_marketplaces.json";
pub(super) const OFFICIAL_MARKETPLACE_NAME: &str = "claude-plugins-official";
pub(super) const OFFICIAL_MARKETPLACE_SOURCE: &str = "anthropics/claude-plugins-official";
pub(super) const REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(
    CONFIG_MD,
    &[
        "claude code",
        "claude-code",
        "claude_code",
        "claudecode",
        "claude",
    ],
);
const COMMAND_MIGRATION_PROFILE: CommandMigrationProfile =
    CommandMigrationProfile::new(REWRITE_PROFILE, CommandDescriptionMode::RequireFrontmatter);

pub(super) fn build_mcp_config(
    source_root: &Path,
    external_agent_home: &Path,
    settings: Option<&JsonValue>,
) -> io::Result<TomlValue> {
    build_mcp_config_from_external(source_root, Some(external_agent_home), settings)
}

pub(super) fn repo_instruction_source_groups(
    repo_root: &Path,
) -> io::Result<Vec<InstructionSourceGroup>> {
    for candidate in [
        repo_root.join(CONFIG_MD),
        repo_root.join(CONFIG_DIR).join(CONFIG_MD),
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

pub(super) fn home_instruction_sources(external_agent_home: &Path) -> io::Result<Vec<PathBuf>> {
    let path = external_agent_home.join(CONFIG_MD);
    Ok(is_non_empty_text_file(&path)?
        .then_some(path)
        .into_iter()
        .collect())
}

pub(super) fn read_instruction_source(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

pub(super) fn import_source_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    import_commands_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn count_missing_source_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<usize> {
    count_missing_commands_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn missing_source_command_names(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    missing_command_names_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn import_source_subagents(
    source_agents: &Path,
    target_agents: &Path,
) -> io::Result<Vec<String>> {
    import_subagents_with_rewrite_profile(source_agents, target_agents, REWRITE_PROFILE)
}

pub(super) fn source_hook_event_names(
    source_dir: &Path,
    target_hooks: &Path,
) -> io::Result<Vec<String>> {
    hook_migration_event_names(source_dir, target_hooks, REWRITE_PROFILE)
}

pub(super) fn import_source_hooks(source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
    import_hooks(source_dir, target_hooks, REWRITE_PROFILE)
}
