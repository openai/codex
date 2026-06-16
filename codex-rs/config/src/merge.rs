use crate::key_aliases::normalize_key_aliases;
use crate::key_aliases::normalized_with_key_aliases;
use codex_network_proxy::normalize_host;
use toml::Value as TomlValue;

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values_at_path(base, overlay, &[]);
}

pub fn merge_toml_values_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &[String]) {
    merge_toml_values_recursive(base, overlay, &mut path.to_vec());
}

fn merge_toml_values_recursive(base: &mut TomlValue, overlay: &TomlValue, path: &mut Vec<String>) {
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
        if is_shell_environment_filters_path(path)
            || cfg!(target_os = "windows") && is_shell_environment_set_path(path)
        {
            normalize_case_insensitive_keys(base_table);
            normalize_case_insensitive_keys(&mut overlay_table);
        }

        for (key, value) in overlay_table {
            path.push(key.clone());
            if let Some(existing) = base_table.get_mut(&key) {
                merge_toml_values_recursive(existing, &value, path);
            } else {
                base_table.insert(key, normalized_for_merge(&value, path));
            }
            path.pop();
        }
    } else {
        *base = normalized_for_merge(overlay, path);
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

    // Ordinary config keeps accepting legacy arrays while `filters` is the
    // canonical keyed form. Reconcile the two shapes only at this boundary so
    // layer precedence remains correct and array compatibility can be removed
    // cleanly after the migration.
    let overlay_has_filters = overlay.contains_key("filters");
    let overlay_has_legacy =
        overlay.contains_key("exclude") || overlay.contains_key("include_only");
    if overlay_has_filters && !overlay_has_legacy {
        convert_legacy_to_filters(base);
    } else if overlay_has_legacy && !overlay_has_filters {
        convert_filters_to_legacy(base);
    }

    for (legacy_field, opposite_field) in [("exclude", "include_only"), ("include_only", "exclude")]
    {
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
        remove_patterns_from_legacy_array(base, opposite_field, &patterns);
    }
}

fn convert_legacy_to_filters(table: &mut toml::map::Map<String, TomlValue>) {
    let mut filters = match table.get("filters") {
        Some(TomlValue::Table(filters)) => filters.clone(),
        Some(_) => return,
        None => toml::map::Map::new(),
    };
    table.remove("filters");
    normalize_case_insensitive_keys(&mut filters);
    for (field, action) in [("exclude", "exclude"), ("include_only", "include")] {
        let Some(TomlValue::Array(patterns)) = table.get(field).cloned() else {
            continue;
        };
        table.remove(field);
        for pattern in patterns {
            if let TomlValue::String(pattern) = pattern {
                filters
                    .entry(pattern.to_ascii_lowercase())
                    .or_insert_with(|| TomlValue::String(action.to_string()));
            }
        }
    }
    if !filters.is_empty() {
        table.insert("filters".to_string(), TomlValue::Table(filters));
    }
}

fn convert_filters_to_legacy(table: &mut toml::map::Map<String, TomlValue>) {
    let Some(TomlValue::Table(mut filters)) = table.get("filters").cloned() else {
        return;
    };
    table.remove("filters");
    normalize_case_insensitive_keys(&mut filters);
    for (pattern, action) in filters {
        match action.as_str() {
            Some("exclude") => push_legacy_pattern(table, "exclude", "include_only", pattern),
            Some("include") => push_legacy_pattern(table, "include_only", "exclude", pattern),
            _ => {}
        }
    }
}

fn push_legacy_pattern(
    table: &mut toml::map::Map<String, TomlValue>,
    field: &str,
    opposite_field: &str,
    pattern: String,
) {
    remove_patterns_from_legacy_array(table, opposite_field, &[pattern.as_str()]);
    let items = table
        .entry(field.to_string())
        .or_insert_with(|| TomlValue::Array(Vec::new()));
    let Some(items) = items.as_array_mut() else {
        return;
    };
    if !items.iter().any(|item| {
        item.as_str()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&pattern))
    }) {
        items.push(TomlValue::String(pattern));
    }
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
        item.as_str().is_none_or(|candidate| {
            !patterns
                .iter()
                .any(|pattern| candidate.eq_ignore_ascii_case(pattern))
        })
    });
}

fn is_shell_environment_filters_path(path: &[String]) -> bool {
    matches!(
        path,
        [policy, filters]
            if policy == "shell_environment_policy" && filters == "filters"
    )
}

fn is_shell_environment_set_path(path: &[String]) -> bool {
    matches!(
        path,
        [policy, set] if policy == "shell_environment_policy" && set == "set"
    )
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

fn normalize_case_insensitive_keys(table: &mut toml::map::Map<String, TomlValue>) {
    let entries = std::mem::take(table);
    for (key, value) in entries {
        table.insert(key.to_ascii_lowercase(), value);
    }
}

fn normalized_for_merge(value: &TomlValue, path: &[String]) -> TomlValue {
    let mut normalized = normalized_with_key_aliases(value, path);
    normalize_nested_case_insensitive_keys(&mut normalized, &mut path.to_vec());
    normalized
}

fn normalize_nested_case_insensitive_keys(value: &mut TomlValue, path: &mut Vec<String>) {
    match value {
        TomlValue::Table(table) => {
            if is_shell_environment_filters_path(path)
                || cfg!(target_os = "windows") && is_shell_environment_set_path(path)
            {
                normalize_case_insensitive_keys(table);
            }
            for (key, value) in table {
                path.push(key.clone());
                normalize_nested_case_insensitive_keys(value, path);
                path.pop();
            }
        }
        TomlValue::Array(items) => {
            for item in items {
                normalize_nested_case_insensitive_keys(item, path);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
