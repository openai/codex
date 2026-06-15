use crate::key_aliases::normalize_key_aliases;
use crate::key_aliases::normalized_with_key_aliases;
use codex_network_proxy::normalize_host;
use toml::Value as TomlValue;

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values_at_path(base, overlay, &mut Vec::new());
}

fn merge_toml_values_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &mut Vec<String>) {
    prepare_shell_environment_policy_merge(base, overlay, path);

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

fn prepare_shell_environment_policy_merge(
    base: &mut TomlValue,
    overlay: &TomlValue,
    path: &[String],
) {
    if !matches!(path, [policy] if policy == "shell_environment_policy") {
        return;
    }
    let TomlValue::Table(base) = base else {
        return;
    };
    let TomlValue::Table(overlay) = overlay else {
        return;
    };

    // Ordinary config keeps accepting legacy arrays while `rules` is the
    // canonical keyed form. Reconcile the two shapes only at this boundary so
    // layer precedence remains correct and array compatibility can be removed
    // cleanly after the migration.
    for (legacy_field, opposite_field, action) in [
        ("exclude", "include_only", "exclude"),
        ("include_only", "exclude", "include"),
    ] {
        let Some(patterns) = overlay
            .get(legacy_field)
            .and_then(TomlValue::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .map(TomlValue::as_str)
                    .collect::<Option<Vec<_>>>()
            })
        else {
            continue;
        };

        if let Some(base_rules) = base.get_mut("rules").and_then(TomlValue::as_table_mut) {
            base_rules.retain(|pattern, value| {
                value.as_str() != Some(action) && !patterns.contains(&pattern)
            });
        }
        remove_patterns_from_legacy_array(base, opposite_field, &patterns);
    }

    let Some(overlay_rules) = overlay.get("rules").and_then(TomlValue::as_table) else {
        return;
    };
    let patterns = overlay_rules.keys().map(String::as_str).collect::<Vec<_>>();
    remove_patterns_from_legacy_array(base, "exclude", &patterns);
    remove_patterns_from_legacy_array(base, "include_only", &patterns);
}

fn remove_patterns_from_legacy_array(
    table: &mut toml::map::Map<String, TomlValue>,
    field: &str,
    patterns: &[&str],
) {
    let Some(items) = table.get_mut(field).and_then(TomlValue::as_array_mut) else {
        return;
    };
    if !items.iter().all(|item| item.as_str().is_some()) {
        return;
    }
    items.retain(|item| {
        item.as_str()
            .is_none_or(|pattern| !patterns.contains(&pattern))
    });
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
