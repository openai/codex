use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::config_types::McpServerConfig;

/// Expand `${VAR}` and `${VAR:-default}` sequences in `input`.
///
/// - `${VAR}`: replaced by `lookup(VAR)` or returns an error if unset.
/// - `${VAR:-default}`: replaced by `lookup(VAR)` if set; otherwise `default`.
///
/// No whitespace is trimmed. Defaults are treated as literal strings (no nested
/// expansions inside the default value). Variable names must match
/// `^[A-Za-z_][A-Za-z0-9_]*$`.
pub(crate) fn expand_vars(
    input: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
    source_label: &str,
) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            // Find closing brace
            let start_inner = i + 2;
            let mut end = start_inner;
            let mut found = false;
            while end < bytes.len() {
                if bytes[end] == b'}' {
                    found = true;
                    break;
                }
                end += 1;
            }
            if !found {
                return Err(anyhow!(
                    "unterminated variable expansion starting at byte {i} in {source_label}"
                ));
            }
            let inner = &input[start_inner..end];
            let (name, default) = match inner.split_once(":-") {
                Some((n, d)) => (n, Some(d)),
                None => (inner, None),
            };

            if !is_valid_var_name(name) {
                return Err(anyhow!(
                    "invalid variable name `{}` in {} (must match ^[A-Za-z_][A-Za-z0-9_]*$)",
                    name,
                    source_label
                ));
            }

            let replacement = match (lookup(name), default) {
                (Some(v), _) => v,
                (None, Some(d)) => d.to_string(),
                (None, None) => {
                    return Err(anyhow!(
                        "environment variable `{}` not set and no default provided in {}",
                        name,
                        source_label
                    ));
                }
            };
            out.push_str(&replacement);
            i = end + 1;
            continue;
        }
        // Copy through single byte as UTF-8 is preserved by slicing boundaries here.
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(out)
}

fn is_valid_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if is_alpha_or_underscore(c) => (),
        _ => return false,
    }
    chars.all(|c| is_alnum_or_underscore(c))
}

fn is_alpha_or_underscore(c: char) -> bool {
    (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c == '_'
}

fn is_alnum_or_underscore(c: char) -> bool {
    is_alpha_or_underscore(c) || (c >= '0' && c <= '9')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_vars_simple() {
        let lookup = |k: &str| match k {
            "USER" => Some("alice".into()),
            _ => None,
        };
        let res = expand_vars("/home/${USER}/bin", lookup, "test");
        match res {
            Ok(s) => assert_eq!(s, "/home/alice/bin"),
            Err(e) => panic!("unexpected error: {e:#}"),
        }
    }

    #[test]
    fn test_expand_vars_with_default() {
        let lookup = |_k: &str| None;
        let res = expand_vars("${REGION:-us-east}", lookup, "test");
        match res {
            Ok(s) => assert_eq!(s, "us-east"),
            Err(e) => panic!("unexpected error: {e:#}"),
        }
    }

    #[test]
    fn test_expand_vars_missing_errors() {
        let lookup = |_k: &str| None;
        let res = expand_vars("x${REQUIRED}y", lookup, "test");
        let msg = match res {
            Ok(v) => panic!("expected error, got {v}"),
            Err(e) => format!("{e:#}"),
        };
        assert!(msg.contains("environment variable `REQUIRED` not set"));
    }

    #[test]
    fn test_expand_vars_multiple() {
        let lookup = |k: &str| match k {
            "A" => Some("1".into()),
            "B" => Some("2".into()),
            _ => None,
        };
        let res = expand_vars("${A}-${B}-${C:-x}", lookup, "test");
        match res {
            Ok(s) => assert_eq!(s, "1-2-x"),
            Err(e) => panic!("unexpected error: {e:#}"),
        }
    }

    #[test]
    fn test_expand_vars_invalid_name() {
        let lookup = |_k: &str| None;
        let res = expand_vars("${1BAD}", lookup, "test");
        let msg = match res {
            Ok(v) => panic!("expected error, got {v}"),
            Err(e) => format!("{e:#}"),
        };
        assert!(msg.contains("invalid variable name"));
    }

    #[test]
    fn test_expand_vars_unterminated() {
        let lookup = |_k: &str| None;
        let res = expand_vars("abc ${FOO", lookup, "test-file");
        let msg = match res {
            Ok(v) => panic!("expected error, got {v}"),
            Err(e) => format!("{e:#}"),
        };
        assert!(msg.contains("unterminated variable expansion"));
        assert!(msg.contains("test-file"));
    }
}

// -------------------------------
// Serde types and converters
// -------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
    Local,
}

#[derive(Debug, Deserialize, Default)]
pub struct McpToml {
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpTomlEntry>,
}

#[derive(Debug, Deserialize, Default)]
pub struct McpTomlEntry {
    #[serde(default)]
    pub r#type: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Convert a permissive TOML entry to the strict `McpServerConfig` used by Codex.
///
/// - Only `stdio` (or missing) transport is accepted; anything else returns an error.
/// - Expands variables in `command`, each `args[]`, and each `env` value.
/// - Returns an error if `command` is missing (after expansion) or if any
///   `${VAR}` expansion fails with no default.
pub fn to_mcp_server_config(
    entry: &McpTomlEntry,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<McpServerConfig> {
    // Transport check: only allow stdio or unspecified
    if let Some(t) = entry.r#type.as_deref() {
        let t_lower = t.to_ascii_lowercase();
        if t_lower != "stdio" {
            return Err(anyhow!(
                "unsupported MCP transport `{}` (only `stdio` supported)",
                t
            ));
        }
    }

    // Command is required
    let command_raw = entry
        .command
        .as_ref()
        .ok_or_else(|| anyhow!("missing `command` for stdio MCP server"))?;
    let command = expand_vars(command_raw, &mut lookup, "overlay:command")?;

    // Args with expansion
    let mut args = Vec::with_capacity(entry.args.len());
    for a in &entry.args {
        args.push(expand_vars(a, &mut lookup, "overlay:args")?);
    }

    // Env values with expansion; keep as None if empty
    let mut env_out: HashMap<String, String> = HashMap::with_capacity(entry.env.len());
    for (k, v) in &entry.env {
        env_out.insert(k.clone(), expand_vars(v, &mut lookup, "overlay:env")?);
    }

    Ok(McpServerConfig {
        command,
        args,
        env: if env_out.is_empty() {
            None
        } else {
            Some(env_out)
        },
    })
}

#[cfg(test)]
mod convert_tests {
    use super::*;

    #[test]
    fn test_to_mcp_server_config_stdio_ok() {
        let entry = McpTomlEntry {
            r#type: None,
            command: Some("${HOME}/bin/svc".to_string()),
            args: vec!["--region".into(), "${REGION:-us-east}".into()],
            env: HashMap::from([(String::from("API_KEY"), String::from("${KEY}"))]),
        };
        let mut map = HashMap::new();
        map.insert("HOME".to_string(), "/home/alice".to_string());
        map.insert("KEY".to_string(), "secret".to_string());
        let lookup = |k: &str| map.get(k).cloned();
        let cfg = match to_mcp_server_config(&entry, lookup) {
            Ok(c) => c,
            Err(e) => panic!("unexpected error: {e:#}"),
        };
        assert_eq!(cfg.command, "/home/alice/bin/svc");
        assert_eq!(cfg.args, vec!["--region", "us-east"]);
        let api_key = cfg.env.as_ref().and_then(|m| m.get("API_KEY")).cloned();
        assert_eq!(api_key.as_deref(), Some("secret"));
    }

    #[test]
    fn test_to_mcp_server_config_reject_non_stdio() {
        for t in ["http", "sse", "HTTP", "SSe"] {
            let entry = McpTomlEntry {
                r#type: Some(t.to_string()),
                command: Some("tool".to_string()),
                ..Default::default()
            };
            let msg = match to_mcp_server_config(&entry, |_k| None) {
                Ok(v) => panic!("expected error, got {v:?}"),
                Err(e) => format!("{e:#}"),
            };
            assert!(msg.to_lowercase().contains("unsupported mcp transport"));
        }
    }

    #[test]
    fn test_to_mcp_server_config_missing_command_errors() {
        let entry = McpTomlEntry {
            command: None,
            ..Default::default()
        };
        let msg = match to_mcp_server_config(&entry, |_k| None) {
            Ok(v) => panic!("expected error, got {v:?}"),
            Err(e) => format!("{e:#}"),
        };
        assert!(msg.contains("missing `command`"));
    }

    #[test]
    fn test_to_mcp_server_config_missing_env_var_errors() {
        let entry = McpTomlEntry {
            command: Some("tool".into()),
            args: vec!["${REQUIRED}".into()],
            ..Default::default()
        };
        let msg = match to_mcp_server_config(&entry, |_k| None) {
            Ok(v) => panic!("expected error, got {v:?}"),
            Err(e) => format!("{e:#}"),
        };
        assert!(msg.contains("environment variable `REQUIRED` not set"));
    }
}

// -------------------------------
// Overlay loader
// -------------------------------

/// Load `.mcp.local.toml` and `.mcp.toml` from `project_root` if they exist.
///
/// Returns the successfully parsed overlays in precedence order: Local then Project.
/// Invalid TOML is logged and skipped.
pub fn load_project_overlays(project_root: &Path) -> Result<Vec<(Scope, McpToml)>> {
    let mut overlays = Vec::new();

    let local_path = project_root.join(".mcp.local.toml");
    if local_path.exists() {
        match std::fs::read_to_string(&local_path) {
            Ok(contents) => match toml::from_str::<McpToml>(&contents) {
                Ok(parsed) => overlays.push((Scope::Local, parsed)),
                Err(e) => tracing::warn!("Failed to parse {}: {e}", local_path.display()),
            },
            Err(e) => tracing::warn!("Failed to read {}: {e}", local_path.display()),
        }
    }

    let project_path = project_root.join(".mcp.toml");
    if project_path.exists() {
        match std::fs::read_to_string(&project_path) {
            Ok(contents) => match toml::from_str::<McpToml>(&contents) {
                Ok(parsed) => overlays.push((Scope::Project, parsed)),
                Err(e) => tracing::warn!("Failed to parse {}: {e}", project_path.display()),
            },
            Err(e) => tracing::warn!("Failed to read {}: {e}", project_path.display()),
        }
    }

    Ok(overlays)
}

#[cfg(test)]
mod overlay_tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_project_overlays_reads_both_files() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path();
        // Pretend it's a git repo to mirror typical layout; not required by loader.
        fs::write(root.join(".git"), "gitdir: nowhere")?;

        // Write project overlay
        fs::write(
            root.join(".mcp.toml"),
            r#"[mcp_servers.alpha]
command = "alpha"
"#,
        )?;

        // Write local overlay
        fs::write(
            root.join(".mcp.local.toml"),
            r#"[mcp_servers.beta]
command = "beta"
"#,
        )?;

        let overlays = load_project_overlays(root)?;
        assert_eq!(overlays.len(), 2);

        // Expect Local first, then Project (our precedence order for merging later)
        assert!(matches!(overlays[0].0, Scope::Local));
        assert!(overlays[0].1.mcp_servers.contains_key("beta"));
        assert!(matches!(overlays[1].0, Scope::Project));
        assert!(overlays[1].1.mcp_servers.contains_key("alpha"));
        Ok(())
    }
}
