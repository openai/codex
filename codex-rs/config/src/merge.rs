use crate::key_aliases::normalize_key_aliases;
use crate::key_aliases::normalized_with_key_aliases;
use codex_network_proxy::normalize_host;
use toml::Value as TomlValue;

/// The mutually exclusive shell-environment filter representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEnvironmentPolicyFilterRepresentation {
    Filters,
    Legacy,
}

impl ShellEnvironmentPolicyFilterRepresentation {
    /// Returns the representation selected by a policy table, including empty fields.
    pub fn from_policy(policy: &TomlValue) -> Option<Self> {
        let policy = policy.as_table()?;
        if policy.contains_key("filters") {
            Some(Self::Filters)
        } else if policy.contains_key("exclude") || policy.contains_key("include_only") {
            Some(Self::Legacy)
        } else {
            None
        }
    }

    /// Returns the representation addressed by a dotted config path.
    pub fn from_path(path: &[String]) -> Option<Self> {
        match path {
            [policy, field, ..] if policy == "shell_environment_policy" => match field.as_str() {
                "filters" => Some(Self::Filters),
                "exclude" | "include_only" => Some(Self::Legacy),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns the representation selected by an edit at `path`.
    pub fn from_edit(path: &[String], value: &TomlValue) -> Option<Self> {
        if matches!(path, [policy] if policy == "shell_environment_policy") {
            Self::from_policy(value)
        } else {
            Self::from_path(path)
        }
    }

    /// Returns the policy fields that must be removed when this representation is selected.
    pub fn displaced_fields(self) -> &'static [&'static str] {
        match self {
            Self::Filters => &["exclude", "include_only"],
            Self::Legacy => &["filters"],
        }
    }
}

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    merge_toml_values_at_path(base, overlay, &mut Vec::new());
}

fn merge_toml_values_at_path(base: &mut TomlValue, overlay: &TomlValue, path: &mut Vec<String>) {
    replace_shell_environment_policy_filter_representation(base, overlay, path);

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
        if is_shell_environment_filters_path(path) {
            normalize_case_insensitive_keys(base_table);
            normalize_case_insensitive_keys(&mut overlay_table);
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

fn is_shell_environment_filters_path(path: &[String]) -> bool {
    matches!(
        path,
        [policy, filters]
            if policy == "shell_environment_policy" && filters == "filters"
    )
}

/// Switching between legacy arrays and keyed filters replaces lower filter
/// fields instead of attempting to reconcile the two representations. Legacy
/// arrays already replace wholesale, so reconciling them would add merge
/// semantics that the legacy representation never supported.
fn replace_shell_environment_policy_filter_representation(
    base: &mut TomlValue,
    overlay: &TomlValue,
    path: &[String],
) {
    if !matches!(path, [policy] if policy == "shell_environment_policy") {
        return;
    }
    let Some(overlay_representation) =
        ShellEnvironmentPolicyFilterRepresentation::from_policy(overlay)
    else {
        return;
    };
    let TomlValue::Table(base) = base else {
        return;
    };

    for field in overlay_representation.displaced_fields() {
        base.remove(*field);
    }
}

/// Looks up a shell-environment filter pattern while ignoring case.
pub fn shell_environment_filter_entry<'a>(
    root: &'a TomlValue,
    path: &[String],
) -> Option<(&'a String, &'a TomlValue)> {
    let [policy, filters, pattern] = path else {
        return None;
    };
    if policy != "shell_environment_policy" || filters != "filters" {
        return None;
    }

    let pattern = pattern.to_lowercase();
    root.get(policy)?
        .get(filters)?
        .as_table()?
        .iter()
        .find(|(candidate, _)| candidate.to_lowercase() == pattern)
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
        table.insert(key.to_lowercase(), value);
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
