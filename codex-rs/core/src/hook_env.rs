use std::collections::HashMap;
use std::fs;

use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

const HOOK_ENV_DIR: &str = "env";

#[derive(Debug)]
pub(crate) struct HookEnvFile {
    path: AbsolutePathBuf,
}

impl HookEnvFile {
    /// Creates the per-thread env file handle and makes its parent directory appendable for hooks.
    pub(crate) fn new(codex_home: &AbsolutePathBuf, thread_id: ThreadId) -> Self {
        let env_file = Self {
            path: codex_home
                .join(HOOK_ENV_DIR)
                .join(format!("{thread_id}.sh")),
        };
        env_file.ensure_parent();
        env_file
    }

    /// Returns the path exposed to hook commands via CODEX_ENV_FILE and CLAUDE_ENV_FILE.
    pub(crate) fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    /// Best-effort directory setup so a hook can append to the advertised env file path.
    fn ensure_parent(&self) {
        let Some(parent) = self.path.as_path().parent() else {
            return;
        };
        if let Err(err) = fs::create_dir_all(parent) {
            warn!(
                path = %parent.display(),
                "failed to create hook env file directory: {err}"
            );
        }
    }

    /// Applies persisted hook env updates to the command environment used by later local tools.
    pub(crate) fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        match fs::read_to_string(self.path.as_path()) {
            Ok(contents) => {
                for line in contents.lines() {
                    apply_env_file_line(env, line);
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                warn!(
                    path = %self.path.display(),
                    "failed to read hook env file: {err}"
                );
            }
        }
    }
}

/// Applies one supported shell-style env update line.
///
/// We intentionally support the common hook outputs here rather than sourcing
/// the file in a hidden shell wrapper: `export NAME=value`, plus Bash's
/// `declare -x NAME=value` form emitted by `export -p`.
fn apply_env_file_line(env: &mut HashMap<String, String>, line: &str) {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return;
    }

    let Some(assignment) = line
        .strip_prefix("export ")
        .or_else(|| line.strip_prefix("declare -x "))
    else {
        return;
    };
    let assignment = assignment.trim_start();
    let Some((name, value)) = assignment.split_once('=') else {
        return;
    };
    let name = name.trim();
    if !is_env_name(name) {
        return;
    }
    let value = parse_env_value(value.trim_start(), env);
    env.insert(name.to_string(), value);
}

/// Parses the value side of an env assignment, preserving single-quoted values literally.
fn parse_env_value(value: &str, env: &HashMap<String, String>) -> String {
    if let Some(value) = value
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        return value.to_string();
    }
    if let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return expand_env_vars(&unescape_double_quoted(value), env);
    }
    expand_env_vars(value.trim_end(), env)
}

/// Handles the small set of backslash escapes expected inside double-quoted env values.
fn unescape_double_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\'
            && let Some(next) = chars.next()
        {
            match next {
                '$' | '"' | '\\' => out.push(next),
                other => {
                    out.push(ch);
                    out.push(other);
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

/// Expands `$NAME` and `${NAME}` using the environment accumulated for this command.
fn expand_env_vars(value: &str, env: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }

        if chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == '}' {
                    break;
                }
                name.push(next);
            }
            if is_env_name(&name) {
                out.push_str(env.get(&name).map_or("", String::as_str));
            } else {
                out.push_str("${");
                out.push_str(&name);
                out.push('}');
            }
            continue;
        }

        let mut name = String::new();
        while let Some(&next) = chars.peek() {
            if !is_env_name_char(next) {
                break;
            }
            chars.next();
            name.push(next);
        }
        if name.is_empty() {
            out.push('$');
        } else if is_env_name(&name) {
            out.push_str(env.get(&name).map_or("", String::as_str));
        } else {
            out.push('$');
            out.push_str(&name);
        }
    }
    out
}

/// Returns whether a string is a valid shell environment variable name.
fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic()) && chars.all(is_env_name_char)
}

/// Returns whether a character is valid after the first character in an env var name.
fn is_env_name_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn apply_env_file_lines(env: &mut HashMap<String, String>, contents: &str) {
        for line in contents.lines() {
            apply_env_file_line(env, line);
        }
    }

    #[test]
    fn env_file_applies_exports_and_expands_path() {
        let mut env = HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]);

        apply_env_file_lines(
            &mut env,
            r#"
# ignored
export FOO=bar
export PATH="/plugin/bin:$PATH"
export LITERAL='$FOO'
"#,
        );

        assert_eq!(
            env,
            HashMap::from([
                ("FOO".to_string(), "bar".to_string()),
                ("LITERAL".to_string(), "$FOO".to_string()),
                ("PATH".to_string(), "/plugin/bin:/usr/bin".to_string()),
            ])
        );
    }

    #[test]
    fn env_file_supports_braced_references_and_declare_exports() {
        let mut env = HashMap::from([("BASE".to_string(), "base".to_string())]);

        apply_env_file_lines(
            &mut env,
            r#"
declare -x FROM_DECLARE="${BASE}/declare"
export FROM_BRACES=${FROM_DECLARE}/braces
"#,
        );

        assert_eq!(
            env,
            HashMap::from([
                ("BASE".to_string(), "base".to_string()),
                ("FROM_DECLARE".to_string(), "base/declare".to_string()),
                ("FROM_BRACES".to_string(), "base/declare/braces".to_string()),
            ])
        );
    }
}
