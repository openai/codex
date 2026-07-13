use codex_core_plugins::marketplace_add::is_local_marketplace_source;
use codex_external_agent_migration::CommandDescriptionMode;
use codex_external_agent_migration::CommandMigrationProfile;
use codex_external_agent_migration::RewriteProfile;
use codex_external_agent_migration::build_mcp_config_from_external;
use codex_external_agent_migration::count_missing_commands_with_profile;
use codex_external_agent_migration::hook_migration_event_names_cla;
use codex_external_agent_migration::import_commands_with_profile;
use codex_external_agent_migration::import_hooks_cla;
use codex_external_agent_migration::import_subagents_with_rewrite_profile;
use codex_external_agent_migration::missing_command_names_with_profile;
use codex_plugin::PluginId;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashSet;
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

pub(super) fn effective_settings(project_settings: &Path) -> io::Result<Option<JsonValue>> {
    let mut effective = super::read_external_settings(project_settings)?;
    let Some(settings_dir) = project_settings.parent() else {
        return Ok(effective);
    };
    let local_settings = settings_dir.join("settings.local.json");
    let local_settings = match super::read_external_settings(&local_settings) {
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

pub(super) fn append_config(
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

pub(super) fn detect_plugins(
    context: &PluginDetectionContext<'_>,
) -> Option<DetectedSourcePlugins> {
    let settings = context.settings?;
    let import_sources =
        marketplace_import_sources(settings, context.external_agent_home, context.source_root);
    let details = extract_plugin_migration_details(
        settings,
        &import_sources,
        context.configured_plugin_ids,
        context.configured_marketplace_plugins,
    )?;
    Some(DetectedSourcePlugins {
        description: format!(
            "Migrate enabled plugins from {}",
            context.source_settings.display()
        ),
        details,
    })
}

pub(super) fn can_detect_plugins(settings: Option<&JsonValue>) -> bool {
    settings.is_some()
}

pub(super) fn marketplace_import_sources(
    settings: &JsonValue,
    external_agent_home: &Path,
    source_root: &Path,
) -> BTreeMap<String, MarketplaceImportSource> {
    let known_marketplaces_path = external_agent_home.join(KNOWN_MARKETPLACES_PATH);
    let known_marketplaces = match super::read_external_settings(&known_marketplaces_path) {
        Ok(known_marketplaces) => known_marketplaces,
        Err(err) => {
            tracing::warn!(
                path = %known_marketplaces_path.display(),
                error = %err,
                "ignoring invalid external agent marketplace registry"
            );
            None
        }
    };
    let mut import_sources = known_marketplaces
        .as_ref()
        .map(|known_marketplaces| {
            collect_marketplace_import_sources(known_marketplaces, external_agent_home)
        })
        .unwrap_or_default();

    if let Some(extra_known_marketplaces) = settings
        .as_object()
        .and_then(|settings| settings.get("extraKnownMarketplaces"))
    {
        let mut scoped_marketplaces = extra_known_marketplaces.clone();
        if let Some(scoped_marketplaces) = scoped_marketplaces.as_object_mut() {
            for (name, scoped_marketplace) in scoped_marketplaces {
                import_sources.remove(name);
                let Some(known_marketplace) = known_marketplaces
                    .as_ref()
                    .and_then(JsonValue::as_object)
                    .and_then(|known_marketplaces| known_marketplaces.get(name))
                else {
                    continue;
                };
                if scoped_marketplace.get("source") != known_marketplace.get("source") {
                    continue;
                }
                let Some(install_location) = known_marketplace
                    .get("installLocation")
                    .and_then(JsonValue::as_str)
                else {
                    continue;
                };
                let install_location = Path::new(install_location);
                let install_location = if install_location.is_absolute() {
                    install_location.to_path_buf()
                } else {
                    external_agent_home.join(install_location)
                };
                let Some(scoped_marketplace) = scoped_marketplace.as_object_mut() else {
                    continue;
                };
                scoped_marketplace.insert(
                    "installLocation".to_string(),
                    JsonValue::String(install_location.display().to_string()),
                );
            }
        }
        import_sources.extend(collect_marketplace_import_sources(
            &scoped_marketplaces,
            source_root,
        ));
    }

    if has_enabled_plugin_for_marketplace(settings, OFFICIAL_MARKETPLACE_NAME)
        && !import_sources.contains_key(OFFICIAL_MARKETPLACE_NAME)
    {
        import_sources.insert(
            OFFICIAL_MARKETPLACE_NAME.to_string(),
            MarketplaceImportSource {
                source: OFFICIAL_MARKETPLACE_SOURCE.to_string(),
                ref_name: None,
            },
        );
    }

    import_sources
}

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
    hook_migration_event_names_cla(source_dir, target_hooks, REWRITE_PROFILE)
}

pub(super) fn import_source_hooks(source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
    import_hooks_cla(source_dir, target_hooks, REWRITE_PROFILE)
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
        (existing, incoming) => {
            *existing = incoming.clone();
        }
    }
}

fn extract_plugin_migration_details(
    settings: &JsonValue,
    import_sources: &BTreeMap<String, MarketplaceImportSource>,
    configured_plugin_ids: &HashSet<String>,
    configured_marketplace_plugins: &BTreeMap<String, HashSet<String>>,
) -> Option<MigrationDetails> {
    let loadable_marketplaces = import_sources
        .iter()
        .filter_map(|(marketplace_name, source)| {
            is_local_marketplace_source(&source.source, source.ref_name.clone())
                .ok()
                .map(|_| marketplace_name.clone())
        })
        .collect::<HashSet<_>>();
    let mut plugins = BTreeMap::new();
    for plugin_id in collect_enabled_plugins(settings)
        .into_iter()
        .filter(|plugin_id| !configured_plugin_ids.contains(plugin_id))
    {
        let Ok(plugin_id) = PluginId::parse(&plugin_id) else {
            continue;
        };
        if let Some(installable_plugins) =
            configured_marketplace_plugins.get(&plugin_id.marketplace_name)
        {
            if !installable_plugins.contains(&plugin_id.plugin_name) {
                tracing::warn!(
                    plugin_id = %plugin_id.as_key(),
                    marketplace_name = %plugin_id.marketplace_name,
                    "enabled external agent plugin was not found in configured marketplace"
                );
                continue;
            }
        } else if !loadable_marketplaces.contains(&plugin_id.marketplace_name) {
            tracing::warn!(
                plugin_id = %plugin_id.as_key(),
                marketplace_name = %plugin_id.marketplace_name,
                "marketplace source was not found for enabled external agent plugin"
            );
            continue;
        }
        let plugin_group = plugins
            .entry(plugin_id.marketplace_name.clone())
            .or_insert_with(|| PluginsMigration {
                marketplace_name: plugin_id.marketplace_name.clone(),
                plugin_names: Vec::new(),
            });
        plugin_group.plugin_names.push(plugin_id.plugin_name);
    }

    let plugins = plugins
        .into_values()
        .filter_map(|mut plugin_group| {
            if plugin_group.plugin_names.is_empty() {
                return None;
            }
            plugin_group.plugin_names.sort();
            Some(plugin_group)
        })
        .collect::<Vec<_>>();
    if plugins.is_empty() {
        return None;
    }

    Some(MigrationDetails {
        plugins,
        ..Default::default()
    })
}

fn collect_enabled_plugins(settings: &JsonValue) -> Vec<String> {
    let Some(enabled_plugins) = settings
        .as_object()
        .and_then(|settings| settings.get("enabledPlugins"))
        .and_then(JsonValue::as_object)
    else {
        return Vec::new();
    };

    enabled_plugins
        .iter()
        .filter_map(|(plugin_key, enabled)| {
            if !enabled.as_bool().unwrap_or(false) {
                return None;
            }
            PluginId::parse(plugin_key)
                .ok()
                .map(|plugin_id| plugin_id.as_key())
        })
        .collect()
}

fn has_enabled_plugin_for_marketplace(settings: &JsonValue, marketplace_name: &str) -> bool {
    collect_enabled_plugins(settings)
        .into_iter()
        .any(|plugin_id| {
            PluginId::parse(&plugin_id)
                .map(|plugin_id| plugin_id.marketplace_name == marketplace_name)
                .unwrap_or(false)
        })
}

fn collect_marketplace_import_sources(
    marketplaces: &JsonValue,
    source_root: &Path,
) -> BTreeMap<String, MarketplaceImportSource> {
    marketplaces
        .as_object()
        .map(|extra_known_marketplaces| {
            extra_known_marketplaces
                .iter()
                .filter_map(|(name, value)| {
                    let source_fields = if let Some(source) = value.get("source")
                        && source.is_object()
                    {
                        source.as_object()?
                    } else {
                        value.as_object()?
                    };
                    let source_kind = source_fields
                        .get("source")
                        .and_then(JsonValue::as_str)
                        .map(str::trim);
                    let declared_source = match source_kind {
                        Some("github") => source_fields.get("repo"),
                        Some("git") => source_fields.get("url"),
                        Some("directory" | "local") => source_fields.get("path"),
                        Some("file" | "url" | "npm" | "settings") => None,
                        Some(_) => source_fields.get("source"),
                        None => source_fields
                            .get("repo")
                            .or_else(|| source_fields.get("url"))
                            .or_else(|| source_fields.get("path"))
                            .or_else(|| value.get("source")),
                    }
                    .and_then(JsonValue::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                    let materialized_source = value
                        .get("installLocation")
                        .and_then(JsonValue::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .and_then(|value| {
                            let path = Path::new(value);
                            let path = if path.is_absolute() {
                                path.to_path_buf()
                            } else {
                                source_root.join(path)
                            };
                            path.is_dir().then(|| path.display().to_string())
                        });
                    let (source, ref_name) = if let Some(source) = declared_source {
                        let source = if matches!(source_kind, Some("directory" | "local")) {
                            let path = Path::new(source);
                            if path.is_absolute() {
                                path.to_path_buf()
                            } else {
                                source_root.join(path)
                            }
                            .display()
                            .to_string()
                        } else {
                            resolve_external_marketplace_source(source, source_root)
                        };
                        let ref_name = source_fields
                            .get("ref")
                            .or_else(|| value.get("ref"))
                            .and_then(JsonValue::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned);
                        (source, ref_name)
                    } else {
                        (materialized_source?, None)
                    };

                    Some((name.clone(), MarketplaceImportSource { source, ref_name }))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_external_marketplace_source(source: &str, source_root: &Path) -> String {
    if !looks_like_relative_local_path(source) {
        return source.to_string();
    }

    source_root.join(source).display().to_string()
}

fn looks_like_relative_local_path(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../") || source == "." || source == ".."
}
