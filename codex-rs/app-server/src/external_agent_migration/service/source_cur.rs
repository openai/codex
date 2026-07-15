use codex_core_plugins::CommandDescriptionMode;
use codex_core_plugins::CommandMigrationProfile;
use codex_core_plugins::CommandRewriteProfile;
use codex_core_plugins::count_missing_commands_with_profile;
use codex_core_plugins::import_commands_with_profile;
use codex_core_plugins::missing_command_names_with_profile;
use codex_external_agent_migration::RewriteProfile;
use codex_external_agent_migration::build_mcp_config_from_json_file;
use codex_external_agent_migration::hook_migration_event_names_cur;
use codex_external_agent_migration::import_hooks_cur;
use codex_external_agent_migration::import_subagents_with_rewrite_profile;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use super::MigrationDetails;
use super::PluginsMigration;
use super::is_non_empty_text_file;
use super::source::DetectedSourcePlugins;
use super::source::InstructionSourceGroup;
use super::source::MarketplaceImportSource;
use super::source::PluginDetectionContext;

pub(super) const CONFIG_DIR: &str = ".cursor";
pub(super) const MIGRATION_SOURCE: &str = "cursor";
const LEGACY_RULES_FILE: &str = ".cursorrules";
pub(super) const HOME_CONFIG_FILE: &str = "cli-config.json";
pub(super) const PROJECT_CONFIG_FILE: &str = "cli.json";
pub(super) const SANDBOX_CONFIG_FILE: &str = "sandbox.json";
pub(super) const HOOKS_CONFIG_FILE: &str = "hooks.json";
const SANDBOX_SETTINGS_KEY: &str = "__cursorSandbox";
const PLUGIN_MARKETPLACE_MANIFEST: &str = ".cursor-plugin/marketplace.json";
pub(super) const REWRITE_PROFILE: RewriteProfile =
    RewriteProfile::new(LEGACY_RULES_FILE, &[]).with_case_sensitive_term_variants(&["Cursor"]);
const COMMAND_MIGRATION_PROFILE: CommandMigrationProfile = CommandMigrationProfile::new(
    CommandRewriteProfile::new(
        REWRITE_PROFILE.doc_file_name(),
        REWRITE_PROFILE.term_variants(),
    )
    .with_case_sensitive_term_variants(REWRITE_PROFILE.case_sensitive_term_variants()),
    CommandDescriptionMode::UseSourceNameFallback,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CachedMarketplacePlugins {
    pub(super) name: String,
    pub(super) source: PathBuf,
    pub(super) plugin_names: Vec<String>,
}

pub(super) fn build_mcp_config(source_dir: &Path) -> io::Result<TomlValue> {
    build_mcp_config_from_json_file(&source_dir.join("mcp.json"))
}

pub(super) fn repo_instruction_source_groups(
    repo_root: &Path,
) -> io::Result<Vec<InstructionSourceGroup>> {
    let source = repo_root.join(LEGACY_RULES_FILE);
    Ok(is_non_empty_text_file(&source)?
        .then(|| InstructionSourceGroup {
            scope: repo_root.to_path_buf(),
            sources: vec![source],
        })
        .into_iter()
        .collect())
}

pub(super) fn read_instruction_source(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

pub(super) fn effective_settings(
    source_dir: &Path,
    source_settings: &Path,
) -> io::Result<Option<JsonValue>> {
    let mut effective = super::read_external_settings(source_settings)?;
    let sandbox_settings = super::read_external_settings(&source_dir.join(SANDBOX_CONFIG_FILE))?;
    if let Some(sandbox_settings) = sandbox_settings {
        let effective = effective.get_or_insert_with(|| JsonValue::Object(serde_json::Map::new()));
        let Some(effective) = effective.as_object_mut() else {
            return Err(super::invalid_data_error(
                "external agent settings root must be an object",
            ));
        };
        effective.insert(SANDBOX_SETTINGS_KEY.to_string(), sandbox_settings);
    }
    Ok(effective)
}

pub(super) fn append_config(
    root: &mut toml::map::Map<String, TomlValue>,
    settings: &serde_json::Map<String, JsonValue>,
) {
    let Some(sandbox) = settings
        .get(SANDBOX_SETTINGS_KEY)
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

pub(super) fn detect_plugins(
    context: &PluginDetectionContext<'_>,
) -> io::Result<Option<DetectedSourcePlugins>> {
    let mut plugins = Vec::new();
    for marketplace in cached_marketplace_plugins(context.external_agent_home)? {
        let configured_marketplace = context
            .configured_marketplace_plugins
            .get(&marketplace.name);
        let plugin_names = marketplace
            .plugin_names
            .into_iter()
            .filter(|plugin_name| {
                !context
                    .configured_plugin_ids
                    .contains(&format!("{plugin_name}@{}", marketplace.name))
                    && configured_marketplace.is_none_or(|plugins| plugins.contains(plugin_name))
            })
            .collect::<Vec<_>>();
        if !plugin_names.is_empty() {
            plugins.push(PluginsMigration {
                marketplace_name: marketplace.name,
                plugin_names,
            });
        }
    }
    if plugins.is_empty() {
        return Ok(None);
    }
    Ok(Some(DetectedSourcePlugins {
        description: format!(
            "Migrate cached plugins from {}",
            context.external_agent_home.join("plugins/cache").display()
        ),
        details: MigrationDetails {
            plugins,
            ..Default::default()
        },
    }))
}

pub(super) fn marketplace_import_sources(
    external_agent_home: &Path,
) -> io::Result<BTreeMap<String, MarketplaceImportSource>> {
    Ok(cached_marketplace_plugins(external_agent_home)?
        .into_iter()
        .map(|marketplace| {
            (
                marketplace.name,
                MarketplaceImportSource {
                    source: marketplace.source.display().to_string(),
                    ref_name: None,
                },
            )
        })
        .collect())
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
    hook_migration_event_names_cur(
        source_dir,
        &source_dir.join(HOOKS_CONFIG_FILE),
        target_hooks,
        REWRITE_PROFILE,
    )
}

pub(super) fn import_source_hooks(source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
    import_hooks_cur(
        source_dir,
        &source_dir.join(HOOKS_CONFIG_FILE),
        target_hooks,
        REWRITE_PROFILE,
    )
}

pub(super) fn cached_marketplace_plugins(
    external_agent_home: &Path,
) -> io::Result<Vec<CachedMarketplacePlugins>> {
    let marketplaces_root = external_agent_home.join("plugins/marketplaces");
    let cache_root = external_agent_home.join("plugins/cache");
    if !marketplaces_root.is_dir() || !cache_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut marketplaces = Vec::new();
    for entry in fs::read_dir(marketplaces_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let marketplace_root = entry.path();
        let manifest_path = marketplace_root.join(PLUGIN_MARKETPLACE_MANIFEST);
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = match fs::read_to_string(&manifest_path) {
            Ok(manifest) => manifest,
            Err(err) => {
                tracing::warn!(
                    path = %manifest_path.display(),
                    error = %err,
                    "ignoring unreadable external marketplace manifest"
                );
                continue;
            }
        };
        let manifest: JsonValue = match serde_json::from_str(&manifest) {
            Ok(manifest) => manifest,
            Err(err) => {
                tracing::warn!(
                    path = %manifest_path.display(),
                    error = %err,
                    "ignoring invalid external marketplace manifest"
                );
                continue;
            }
        };
        let Some(name) = manifest.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let available_plugins = manifest
            .get("plugins")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(|plugin| plugin.get("name").and_then(JsonValue::as_str))
            .collect::<BTreeSet<_>>();
        let cache_marketplace = cache_root.join(entry.file_name());
        if !cache_marketplace.is_dir() {
            continue;
        }
        let mut plugin_names = fs::read_dir(cache_marketplace)?
            .filter_map(Result::ok)
            .filter_map(|plugin| {
                plugin
                    .file_type()
                    .ok()
                    .filter(std::fs::FileType::is_dir)
                    .and_then(|_| plugin.file_name().into_string().ok())
            })
            .filter(|plugin_name| available_plugins.contains(plugin_name.as_str()))
            .collect::<Vec<_>>();
        plugin_names.sort();
        if !plugin_names.is_empty() {
            marketplaces.push(CachedMarketplacePlugins {
                name: name.to_string(),
                source: marketplace_root,
                plugin_names,
            });
        }
    }
    marketplaces.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(marketplaces)
}

#[cfg(test)]
#[path = "source_cur_tests.rs"]
mod tests;
