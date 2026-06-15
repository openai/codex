use crate::key_aliases::normalize_key_aliases;
use crate::key_aliases::normalized_with_key_aliases;
use codex_network_proxy::normalize_host;
use toml::Value as TomlValue;

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values_at_path(base, overlay, &mut Vec::new());
}

fn merge_toml_values_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &mut Vec<String>) {
    // Ordinary config temporarily accepts legacy arrays and keyed boolean maps
    // for these fields. Promote a lower array only when a map overlays it so
    // tombstones can remove individual entries. Keeping this migration rule
    // narrow preserves legacy array replacement and makes arrays easy to
    // deprecate later without changing the general TOML merge semantics.
    if is_shell_environment_pattern_list_path(path) && overlay.is_table() {
        promote_string_array_to_bool_map(base);
    }

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

fn is_shell_environment_pattern_list_path(path: &[String]) -> bool {
    matches!(
        path,
        [policy, field]
            if policy == "shell_environment_policy"
                && (field == "exclude" || field == "include_only")
    )
}

fn promote_string_array_to_bool_map(value: &mut TomlValue) {
    let TomlValue::Array(items) = value else {
        return;
    };
    let Some(patterns) = items
        .iter()
        .map(TomlValue::as_str)
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    *value = TomlValue::Table(
        patterns
            .into_iter()
            .map(|pattern| (pattern.to_string(), TomlValue::Boolean(true)))
            .collect(),
    );
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
