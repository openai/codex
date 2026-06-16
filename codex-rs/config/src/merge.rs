use crate::key_aliases::normalize_key_aliases;
use crate::key_aliases::normalized_with_key_aliases;
use codex_network_proxy::normalize_host;
use toml::Value as TomlValue;

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values_at_path(base, overlay, &mut Vec::new());
}

fn merge_toml_values_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &mut Vec<String>) {
    reconcile_shell_environment_policy_representations(base, overlay, path);

    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        normalize_key_aliases(path, base_table);
        let mut overlay_table = overlay_table.clone();
        normalize_key_aliases(path, &mut overlay_table);
        normalize_merge_table_keys(path, base_table);
        normalize_merge_table_keys(path, &mut overlay_table);

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

/// Ordinary config keeps accepting legacy arrays while `filters` is the
/// canonical keyed form. Reconcile the two shapes only at this boundary so
/// layer precedence remains correct and array compatibility can be removed
/// cleanly after the migration.
fn reconcile_shell_environment_policy_representations(
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
    normalize_table_keys(&mut filters, |key| key.to_ascii_lowercase());
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
    normalize_table_keys(&mut filters, |key| key.to_ascii_lowercase());
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

/// Canonicalizes keys for maps whose keys compare independently of TOML's
/// case-sensitive key semantics, so equivalent entries collide before merging.
fn normalize_merge_table_keys(path: &[String], table: &mut toml::map::Map<String, TomlValue>) {
    match path {
        [permissions, _, network, domains]
            if permissions == "permissions" && network == "network" && domains == "domains" =>
        {
            normalize_table_keys(table, normalize_host);
        }
        [policy, filters] if policy == "shell_environment_policy" && filters == "filters" => {
            // Environment-variable patterns compare case-insensitively.
            normalize_table_keys(table, |key| key.to_ascii_lowercase());
        }
        _ => {}
    }
}

fn normalize_table_keys(
    table: &mut toml::map::Map<String, TomlValue>,
    normalize_key: impl Fn(&str) -> String,
) {
    let entries = std::mem::take(table);
    for (key, value) in entries {
        table.insert(normalize_key(&key), value);
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
