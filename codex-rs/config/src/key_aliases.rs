//! Normalize config aliases before merging layers and reporting origins.

use toml::Value as TomlValue;
use toml::map::Map as TomlMap;

#[derive(Debug, Clone, Copy)]
struct ConfigKeyAlias {
    legacy: &'static [&'static str],
    canonical: &'static [&'static str],
}

const CONFIG_KEY_ALIASES: &[ConfigKeyAlias] = &[
    ConfigKeyAlias {
        legacy: &["tui", "whimsy"],
        canonical: &["tui", "effects", "starfield"],
    },
    ConfigKeyAlias {
        legacy: &["memories", "no_memories_if_mcp_or_web_search"],
        canonical: &["memories", "disable_on_external_context"],
    },
    ConfigKeyAlias {
        legacy: &["agents", "max_threads"],
        canonical: &["agents", "max_concurrent_threads_per_session"],
    },
];

fn normalize_table_key_aliases(path: &[String], table: &mut TomlMap<String, TomlValue>) {
    'aliases: for alias in CONFIG_KEY_ALIASES {
        // Destinations stay within the legacy key's containing table.
        let Some((legacy_key, table_path)) = alias.legacy.split_last() else {
            continue;
        };
        if path
            .iter()
            .map(String::as_str)
            .eq(table_path.iter().copied())
            && let Some(canonical_path) = alias.canonical.strip_prefix(table_path)
            && let Some(value) = table.remove(*legacy_key)
        {
            // Insert at the canonical path if absent.
            let Some((key, parents)) = canonical_path.split_last() else {
                continue;
            };
            let mut destination = &mut *table;
            for parent in parents {
                let child = destination
                    .entry((*parent).to_string())
                    .or_insert_with(|| TomlValue::Table(TomlMap::new()));
                let Some(child) = child.as_table_mut() else {
                    continue 'aliases;
                };
                destination = child;
            }
            destination.entry((*key).to_string()).or_insert(value);
        }
    }
}

pub(crate) fn normalize_key_aliases(value: &TomlValue) -> TomlValue {
    normalize_key_aliases_at_path(value, &[])
}

fn normalize_key_aliases_at_path(value: &TomlValue, path: &[String]) -> TomlValue {
    match value {
        TomlValue::Table(table) => {
            let mut normalized = TomlMap::new();
            for (key, child) in table {
                let mut child_path = path.to_vec();
                child_path.push(key.clone());
                normalized.insert(
                    key.clone(),
                    normalize_key_aliases_at_path(child, &child_path),
                );
            }
            normalize_table_key_aliases(path, &mut normalized);
            TomlValue::Table(normalized)
        }
        TomlValue::Array(items) => TomlValue::Array(
            items
                .iter()
                .map(|item| normalize_key_aliases_at_path(item, path))
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// Normalize only base tables reached by the overlay, leaving unrelated raw config intact.
///
/// Config-edit table Upserts start from the raw user layer, not the normalized effective config.
/// They use a sparse overlay and persist only the edited subtree, but compute the returned
/// version from the whole in-memory config. Normalizing unrelated base tables would make
/// that version disagree with the file on disk.
///
/// For example, given `[memories] no_memories_if_mcp_or_web_search = false` and
/// `[agents] max_depth = 1`, upserting `agents` with `{ "max_depth": 2 }` must leave
/// the memories alias untouched. Renaming it only in memory would cause the next write
/// using the returned version to fail with `ConfigVersionConflict`, despite no intervening edit.
pub(crate) fn normalize_base_key_aliases(
    base: &mut TomlValue,
    overlay: &TomlValue,
    path: &mut Vec<String>,
) {
    if let TomlValue::Table(base) = base
        && let TomlValue::Table(overlay) = overlay
    {
        normalize_table_key_aliases(path, base);
        for (key, value) in overlay {
            if let Some(existing) = base.get_mut(key) {
                path.push(key.clone());
                normalize_base_key_aliases(existing, value, path);
                path.pop();
            }
        }
    }
}
