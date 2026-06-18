use codex_shell_command::bash::try_parse_shell;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;
use std::time::Duration;

static WALL_TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r"(?m)^(?:Wall time|Duration): (?<seconds>-?[0-9]+(?:\.[0-9]+)?) seconds\r?$")
});
static RUNNING_SESSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r"(?m)^Process running with session ID (?<session_id>-?[0-9]+)\r?$")
});

fn compile_regex(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("invalid slow-command regex `{pattern}`: {error}"),
    }
}

pub(super) fn parse_direct_command(name: &str, arguments: &str) -> Option<String> {
    let value: Value = serde_json::from_str(arguments).ok()?;
    let command = match name {
        "shell_command" => value.get("command"),
        "exec_command" => value.get("cmd").or_else(|| value.get("command")),
        _ => None,
    }?;
    match command {
        Value::String(command) if !command.is_empty() => Some(command.clone()),
        Value::Array(parts) => {
            let parts = parts
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()?;
            (!parts.is_empty()).then(|| {
                codex_shell_command::parse_command::shlex_join(
                    &parts.into_iter().map(str::to_string).collect::<Vec<_>>(),
                )
            })
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) | Value::String(_) => {
            None
        }
    }
}

pub(super) fn parse_session_id(arguments: &str) -> Option<i64> {
    serde_json::from_str::<Value>(arguments)
        .ok()?
        .get("session_id")?
        .as_i64()
}

pub(super) fn parse_wall_time(output: &str) -> Option<Duration> {
    let seconds = WALL_TIME_RE
        .captures(output)?
        .name("seconds")?
        .as_str()
        .parse::<f64>()
        .ok()?;
    Duration::try_from_secs_f64(seconds).ok()
}

pub(super) fn parse_running_session_id(output: &str) -> Option<i64> {
    RUNNING_SESSION_RE
        .captures(output)?
        .name("session_id")?
        .as_str()
        .parse()
        .ok()
}

pub(super) fn first_executable(command: &str) -> String {
    let parsed = try_parse_shell(command).and_then(|tree| {
        let mut nodes = vec![tree.root_node()];
        let mut command_name: Option<(usize, String)> = None;
        while let Some(node) = nodes.pop() {
            if node.kind() == "command_name"
                && command_name
                    .as_ref()
                    .is_none_or(|(start_byte, _name)| node.start_byte() < *start_byte)
                && let Ok(name) = node.utf8_text(command.as_bytes())
            {
                command_name = Some((node.start_byte(), name.to_string()));
            }
            let mut cursor = node.walk();
            nodes.extend(node.children(&mut cursor));
        }
        command_name.map(|(_start_byte, name)| name)
    });
    let executable = parsed.or_else(|| {
        command
            .split_whitespace()
            .find(|token| *token != "&" && !looks_like_environment_assignment(token))
            .map(str::to_string)
    });
    normalize_executable(executable.as_deref().unwrap_or("<unknown>"))
}

fn looks_like_environment_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(name, _value)| {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

pub(super) fn normalize_executable(executable: &str) -> String {
    let executable = executable.trim_matches(['\'', '"']);
    let executable = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    let suffix = executable.get(executable.len().saturating_sub(4)..);
    if executable.len() > 4 && suffix.is_some_and(|suffix| suffix.eq_ignore_ascii_case(".exe")) {
        executable[..executable.len() - 4].to_string()
    } else if executable.is_empty() {
        "<unknown>".to_string()
    } else {
        executable.to_string()
    }
}
