use std::collections::BTreeSet;
use std::collections::HashMap;

use serde_json::json;
use toml::Value as TomlValue;

pub const KEY_COLLISION_SENTINEL: &str = "KEY_COLLISION";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigExpansionWarning {
    pub var: String,
    pub path: String,
}

impl ConfigExpansionWarning {
    pub fn new(var: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            var: var.into(),
            path: path.into(),
        }
    }
}

pub(crate) trait EnvProvider {
    fn get(&self, key: &str) -> Option<String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct RealEnv;

impl EnvProvider for RealEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathSegment {
    Key(String),
    Index(usize),
}

pub(crate) struct ExpansionResult {
    pub value: TomlValue,
    pub warnings: Vec<ConfigExpansionWarning>,
}

pub(crate) fn expand_config_toml(value: TomlValue) -> ExpansionResult {
    expand_config_toml_with_env(value, &RealEnv)
}

pub(crate) fn expand_key_for_matching_with_env(key: &str, env: &impl EnvProvider) -> String {
    if key == "~" {
        let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        if let Some(home) = env.get(home_var) {
            return home;
        }
        return key.to_string();
    }
    let mut warnings = Vec::new();
    expand_string(key, "<projects>", env, &mut warnings)
}

pub(crate) fn expand_config_toml_with_env(
    value: TomlValue,
    env: &impl EnvProvider,
) -> ExpansionResult {
    let mut warnings = Vec::new();
    let mut path = Vec::new();
    let value = expand_value(value, &mut path, env, &mut warnings);
    ExpansionResult { value, warnings }
}

fn expand_value(
    value: TomlValue,
    path: &mut Vec<PathSegment>,
    env: &impl EnvProvider,
    warnings: &mut Vec<ConfigExpansionWarning>,
) -> TomlValue {
    match value {
        TomlValue::String(value) => {
            let path_display = format_path(path);
            TomlValue::String(expand_string(&value, &path_display, env, warnings))
        }
        TomlValue::Array(values) => {
            let mut expanded = Vec::with_capacity(values.len());
            for (index, value) in values.into_iter().enumerate() {
                path.push(PathSegment::Index(index));
                expanded.push(expand_value(value, path, env, warnings));
                path.pop();
            }
            TomlValue::Array(expanded)
        }
        TomlValue::Table(values) => {
            let mut expanded = toml::map::Map::new();
            let mut original_key_by_expanded_key: HashMap<String, String> = HashMap::new();
            for (key, value) in values {
                path.push(PathSegment::Key(key.clone()));
                let parent_path_display = format_path(&path[..path.len().saturating_sub(1)]);
                let key_path_display = format_path(path);
                let expanded_key = expand_string(&key, &key_path_display, env, warnings);
                let expanded_value = expand_value(value, path, env, warnings);
                path.pop();
                if let Some(previous_original_key) =
                    original_key_by_expanded_key.get(&expanded_key).cloned()
                {
                    warnings.push(new_key_collision_warning(
                        parent_path_display,
                        expanded_key,
                        previous_original_key,
                        key,
                    ));
                    continue;
                }
                original_key_by_expanded_key.insert(expanded_key.clone(), key);
                expanded.insert(expanded_key, expanded_value);
            }
            TomlValue::Table(expanded)
        }
        value => value,
    }
}

fn new_key_collision_warning(
    parent_path: String,
    expanded_key: String,
    original_key_a: String,
    original_key_b: String,
) -> ConfigExpansionWarning {
    // Encode collision details into the existing warning shape to avoid a breaking API change.
    let path = json!({
        "path": parent_path,
        "expanded_key": expanded_key,
        "original_keys": [original_key_a, original_key_b],
    })
    .to_string();
    ConfigExpansionWarning::new(KEY_COLLISION_SENTINEL, path)
}

fn expand_string(
    input: &str,
    path: &str,
    env: &impl EnvProvider,
    warnings: &mut Vec<ConfigExpansionWarning>,
) -> String {
    let mut missing_vars = BTreeSet::new();
    let input = expand_tilde_prefix(input, env, &mut missing_vars);
    let mut output = String::with_capacity(input.len());
    let mut iter = input.chars().peekable();

    while let Some(ch) = iter.next() {
        if ch != '$' {
            output.push(ch);
            continue;
        }

        let Some(&next) = iter.peek() else {
            output.push('$');
            continue;
        };

        if next == '$' {
            output.push('$');
            iter.next();
            continue;
        }

        if next == '{' {
            iter.next();
            let mut name = String::new();
            let mut closed = false;
            while let Some(&ch) = iter.peek() {
                iter.next();
                if ch == '}' {
                    closed = true;
                    break;
                }
                name.push(ch);
            }

            if !closed || !is_valid_env_name(&name) {
                output.push_str("${");
                output.push_str(&name);
                if closed {
                    output.push('}');
                }
                continue;
            }

            if let Some(value) = env.get(&name) {
                output.push_str(&value);
            } else {
                missing_vars.insert(name.clone());
                output.push_str("${");
                output.push_str(&name);
                output.push('}');
            }
            continue;
        }

        if !is_valid_env_start(next) {
            output.push('$');
            continue;
        }

        let mut name = String::new();
        name.push(next);
        iter.next();
        while let Some(&ch) = iter.peek() {
            if is_valid_env_continue(ch) {
                name.push(ch);
                iter.next();
            } else {
                break;
            }
        }

        if let Some(value) = env.get(&name) {
            output.push_str(&value);
        } else {
            missing_vars.insert(name.clone());
            output.push('$');
            output.push_str(&name);
        }
    }

    for var in missing_vars {
        warnings.push(ConfigExpansionWarning::new(var, path.to_string()));
    }

    output
}

fn expand_tilde_prefix(
    input: &str,
    env: &impl EnvProvider,
    missing_vars: &mut BTreeSet<String>,
) -> String {
    if !input.starts_with("~/") && !input.starts_with("~\\") {
        return input.to_string();
    }

    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    match env.get(home_var) {
        Some(home) => {
            let mut expanded = String::with_capacity(home.len() + input.len().saturating_sub(1));
            expanded.push_str(&home);
            expanded.push_str(&input[1..]);
            expanded
        }
        None => {
            missing_vars.insert(home_var.to_string());
            input.to_string()
        }
    }
}

fn is_valid_env_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_valid_env_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !is_valid_env_start(first) {
        return false;
    }
    chars.all(is_valid_env_continue)
}

fn format_path(segments: &[PathSegment]) -> String {
    if segments.is_empty() {
        return "<root>".to_string();
    }

    let mut output = String::new();
    for segment in segments {
        match segment {
            PathSegment::Key(key) => {
                if is_simple_key(key) {
                    if !output.is_empty() {
                        output.push('.');
                    }
                    output.push_str(key);
                } else {
                    output.push_str("[\"");
                    output.push_str(&escape_key(key));
                    output.push_str("\"]");
                }
            }
            PathSegment::Index(index) => {
                output.push('[');
                output.push_str(&index.to_string());
                output.push(']');
            }
        }
    }
    output
}

fn is_simple_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn escape_key(key: &str) -> String {
    let mut escaped = String::with_capacity(key.len());
    for ch in key.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
pub(crate) struct FakeEnv {
    vars: std::collections::HashMap<String, String>,
}

#[cfg(test)]
impl FakeEnv {
    pub(crate) fn new(
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            vars: vars
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl EnvProvider for FakeEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }
}
