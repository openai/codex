use crate::key_aliases::normalize_key_aliases;
use crate::key_aliases::normalized_with_key_aliases;
use codex_network_proxy::normalize_host;
use toml::Value as TomlValue;

const ATOMIC_REQUIREMENT_PATHS: &[&[&str]] =
    &[&["mcp_servers", "*"], &["plugins", "*", "mcp_servers", "*"]];

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values_at_path(base, overlay, &mut Vec::new());
}

/// Merge a requirements layer while treating selected requirement values as
/// atomic.
///
/// The regular TOML merge recursively combines tables. Each named MCP server
/// requirement instead represents one complete requirement, so combining its
/// internal fields across layers could retain parts of both definitions.
/// After the regular merge, reapply higher-priority values at the configured
/// atomic paths so each same-name requirement is replaced as a whole. An
/// explicitly empty atomic map clears lower-priority entries. A `*` path
/// segment matches every key at that level.
pub(crate) fn merge_requirements_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values(base, overlay);
    for path in ATOMIC_REQUIREMENT_PATHS {
        apply_atomic_override_at_path(base, overlay, path);
    }
}

fn merge_toml_values_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &mut Vec<String>) {
    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        normalize_key_aliases(path, base_table);
        let mut overlay_table = overlay_table.clone();
        normalize_key_aliases(path, &mut overlay_table);
        if is_permission_network_domains_path(path) {
            normalize_network_domain_keys(base_table);
            normalize_network_domain_keys(&mut overlay_table);
        }

        for (key, value) in overlay_table {
            path.push(key.clone());
            if let Some(existing) = base_table.get_mut(&key) {
                merge_toml_values_at_path(existing, &value, path);
            } else {
                base_table.insert(key, normalized_with_key_aliases(&value, path));
            }
            path.pop();
        }
    } else {
        *base = normalized_with_key_aliases(overlay, path);
    }
}

fn apply_atomic_override_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &[&str]) {
    let Some((segment, remaining)) = path.split_first() else {
        *base = overlay.clone();
        return;
    };
    let Some(overlay_table) = overlay.as_table() else {
        return;
    };
    if *segment == "*" && remaining.is_empty() && overlay_table.is_empty() {
        *base = overlay.clone();
        return;
    }
    let Some(base_table) = base.as_table_mut() else {
        return;
    };

    if *segment == "*" {
        for (key, overlay_value) in overlay_table {
            let Some(base_value) = base_table.get_mut(key) else {
                continue;
            };
            apply_atomic_override_at_path(base_value, overlay_value, remaining);
        }
    } else if let Some(base_value) = base_table.get_mut(*segment)
        && let Some(overlay_value) = overlay_table.get(*segment)
    {
        apply_atomic_override_at_path(base_value, overlay_value, remaining);
    }
}

fn is_permission_network_domains_path(path: &[String]) -> bool {
    matches!(
        path,
        [permissions, _, network, domains]
            if permissions == "permissions" && network == "network" && domains == "domains"
    )
}

fn normalize_network_domain_keys(table: &mut toml::map::Map<String, TomlValue>) {
    let entries = std::mem::take(table);
    for (pattern, value) in entries {
        table.insert(normalize_host(&pattern), value);
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
