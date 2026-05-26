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
    /// Creates the per-thread env file handle without touching the filesystem.
    ///
    /// The hook runtime creates the parent directory only when it exposes this path to a
    /// SessionStart hook. Later tool executions can still call `apply_to_env` safely when the
    /// file was never created.
    pub(crate) fn new(codex_home: &AbsolutePathBuf, thread_id: ThreadId) -> Self {
        Self {
            path: codex_home
                .join(HOOK_ENV_DIR)
                .join(format!("{thread_id}.sh")),
        }
    }

    /// Returns the path exposed to hook commands via CODEX_ENV_FILE and CLAUDE_ENV_FILE.
    pub(crate) fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    /// Applies persisted hook env updates to later local tool environments.
    ///
    /// The returned names are the variables touched by the file. Shell-like tools use that list
    /// to keep a shell snapshot from undoing hook updates such as `PATH` prepends.
    pub(crate) fn apply_to_env(&self, env: &mut HashMap<String, String>) -> Vec<String> {
        match fs::read_to_string(self.path.as_path()) {
            Ok(contents) => {
                let mut applied_names = Vec::new();
                for line in contents.lines() {
                    if let Some(name) = apply_env_file_line(env, line)
                        && !applied_names.contains(&name)
                    {
                        applied_names.push(name);
                    }
                }
                applied_names
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                warn!(
                    path = %self.path.display(),
                    "failed to read hook env file: {err}"
                );
                Vec::new()
            }
        }
    }
}

/// Applies one supported shell-style env update line.
///
/// We intentionally support the common hook outputs here rather than sourcing
/// the file in a hidden shell wrapper: `export NAME=value`, plus Bash's
/// `declare -x NAME=value` form emitted by `export -p`.
fn apply_env_file_line(env: &mut HashMap<String, String>, line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let assignment = line
        .strip_prefix("export ")
        .or_else(|| line.strip_prefix("declare -x "))?;
    let assignment = assignment.trim_start();
    let (name, value) = assignment.split_once('=')?;
    let name = name.trim();
    if !is_env_name(name) {
        return None;
    }
    let value = parse_env_value(value.trim_start(), env);
    env.insert(name.to_string(), value);
    Some(name.to_string())
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

    fn apply_env_file_lines(env: &mut HashMap<String, String>, contents: &str) -> Vec<String> {
        let mut applied_names = Vec::new();
        for line in contents.lines() {
            if let Some(name) = apply_env_file_line(env, line) {
                applied_names.push(name);
            }
        }
        applied_names
    }

    #[test]
    fn env_file_applies_exports_and_expands_path() {
        let mut env = HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]);

        let applied_names = apply_env_file_lines(
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
        assert_eq!(
            applied_names,
            vec!["FOO".to_string(), "PATH".to_string(), "LITERAL".to_string()]
        );
    }

    #[test]
    fn env_file_supports_braced_references_and_declare_exports() {
        let mut env = HashMap::from([("BASE".to_string(), "base".to_string())]);

        let applied_names = apply_env_file_lines(
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
        assert_eq!(
            applied_names,
            vec!["FROM_DECLARE".to_string(), "FROM_BRACES".to_string()]
        );
    }
}
