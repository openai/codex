use crate::CurSource;
use crate::RewriteProfile;
use codex_core_plugins::CommandDescriptionMode;
use codex_core_plugins::CommandMigrationProfile;
use codex_core_plugins::CommandRewriteProfile;
use codex_core_plugins::count_missing_commands_with_profile;
use codex_core_plugins::import_commands_with_profile;
use codex_core_plugins::missing_command_names_with_profile;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::migration_source::MarketplaceImportSource;

const PLUGIN_MARKETPLACE_MANIFEST: &str = ".cursor-plugin/marketplace.json";
pub(super) const REWRITE_PROFILE: RewriteProfile = CurSource::REWRITE_PROFILE;
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

pub(crate) fn cached_marketplace_plugins(
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
