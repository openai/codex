use crate::invalid_data_error;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;

pub(super) const SOURCE_EXTERNAL_AGENT_NAME: &str = "claude";
pub(super) const EXTERNAL_AGENT_HOOKS_SUBDIR: &str = "hooks";
pub(super) const EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR: &str = "hooks";

pub(crate) fn write_hook_migration(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
    migration: serde_json::Map<String, JsonValue>,
) -> io::Result<bool> {
    if migration.is_empty() || !is_missing_or_empty_text_file(target_hooks)? {
        return Ok(false);
    }
    let Some(parent) = target_hooks.parent() else {
        return Err(invalid_data_error("hooks target path has no parent"));
    };
    fs::create_dir_all(parent)?;
    copy_hook_scripts(source_external_agent_dir, parent)?;
    let mut payload = serde_json::Map::new();
    payload.insert("hooks".to_string(), JsonValue::Object(migration));
    let rendered = serde_json::to_string_pretty(&JsonValue::Object(payload))
        .map_err(|err| invalid_data_error(format!("failed to serialize hooks.json: {err}")))?;
    fs::write(target_hooks, format!("{rendered}\n"))?;
    Ok(true)
}

pub(crate) fn rewrite_hook_command_for_source(
    command: &str,
    target_config_dir: Option<&Path>,
    source_external_agent_dir: &Path,
) -> String {
    let Some(target_config_dir) = target_config_dir else {
        return command.to_string();
    };
    if looks_like_windows_hook_command(command) {
        return command.to_string();
    }
    let target_hooks_dir = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
    let source_config_dir = source_external_agent_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(external_agent_config_dir);
    let source_hooks_path = format!("{source_config_dir}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/");
    let command = replace_quoted_hook_paths(command, '\'', &source_hooks_path, &target_hooks_dir);
    let command = replace_quoted_hook_paths(&command, '"', &source_hooks_path, &target_hooks_dir);
    replace_unquoted_hook_paths(&command, &source_hooks_path, &target_hooks_dir)
}

fn replace_quoted_hook_paths(
    command: &str,
    quote: char,
    source_hooks_path: &str,
    target_hooks_dir: &Path,
) -> String {
    let mut rewritten = command.to_string();
    let mut search_start = 0usize;
    while let Some(relative_start) = rewritten[search_start..].find(quote) {
        let start = search_start + relative_start;
        let content_start = start + quote.len_utf8();
        let Some(relative_end) = rewritten[content_start..].find(quote) else {
            break;
        };
        let end = content_start + relative_end;
        let content = &rewritten[content_start..end];
        if let Some(source_hooks_start) = content.find(source_hooks_path) {
            let suffix_start = source_hooks_start + source_hooks_path.len();
            let suffix = &content[suffix_start..];
            let Some(replacement) =
                target_hook_path_replacement(target_hooks_dir, content, source_hooks_start, suffix)
            else {
                search_start = end + quote.len_utf8();
                continue;
            };
            rewritten.replace_range(start..end + quote.len_utf8(), &replacement);
            search_start = start + replacement.len();
        } else {
            search_start = end + quote.len_utf8();
        }
    }
    rewritten
}

fn replace_unquoted_hook_paths(
    command: &str,
    source_hooks_path: &str,
    target_hooks_dir: &Path,
) -> String {
    let mut rewritten = command.to_string();
    let mut search_start = 0usize;
    while let Some(source_hooks_start) =
        find_unquoted_source_hook_path(&rewritten, source_hooks_path, search_start)
    {
        let path_start = shell_path_start(&rewritten, source_hooks_start);
        let path_end = shell_path_end(&rewritten, source_hooks_start + source_hooks_path.len());
        if is_assignment_value_start(&rewritten, path_start) {
            search_start = source_hooks_start + source_hooks_path.len();
            continue;
        }
        let path = rewritten[path_start..path_end].to_string();
        let suffix = rewritten[source_hooks_start + source_hooks_path.len()..path_end].to_string();
        if let Some(replacement) = target_hook_path_replacement(
            target_hooks_dir,
            &path,
            source_hooks_start - path_start,
            &suffix,
        ) {
            rewritten.replace_range(path_start..path_end, &replacement);
            search_start = path_start + replacement.len();
        } else {
            search_start = source_hooks_start + source_hooks_path.len();
        }
    }
    rewritten
}

fn find_unquoted_source_hook_path(
    command: &str,
    source_hooks_path: &str,
    start: usize,
) -> Option<usize> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;
    for (offset, ch) in command[start..].char_indices() {
        let index = start + offset;
        if escaped {
            escaped = false;
            continue;
        }
        if !in_single_quote && ch == '\\' {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            _ if !in_single_quote
                && !in_double_quote
                && command[index..].starts_with(source_hooks_path) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn is_pure_shell_path_content(content: &str, source_hooks_start: usize) -> bool {
    let prefix = &content[..source_hooks_start];
    (prefix.is_empty() || prefix == "./" || prefix.ends_with('/'))
        && !prefix.chars().any(is_shell_path_boundary)
}

fn shell_path_start(command: &str, end: usize) -> usize {
    command[..end]
        .char_indices()
        .filter_map(|(index, ch)| is_shell_path_boundary(ch).then_some(index + ch.len_utf8()))
        .next_back()
        .unwrap_or(0)
}

fn shell_path_end(command: &str, start: usize) -> usize {
    let mut escaped = false;
    for (offset, ch) in command[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if is_shell_path_boundary(ch) {
            return start + offset;
        }
    }
    command.len()
}

fn is_shell_path_boundary(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '=' | ';' | '|' | '&' | '<' | '>' | '(' | ')')
}

fn is_assignment_value_start(command: &str, path_start: usize) -> bool {
    command[..path_start]
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '=')
}

fn target_hook_path_replacement(
    target_hooks_dir: &Path,
    path: &str,
    source_hooks_start: usize,
    suffix: &str,
) -> Option<String> {
    if !is_pure_shell_path_content(path, source_hooks_start) || !is_static_hook_path_suffix(suffix)
    {
        return None;
    }
    Some(shell_single_quote(
        target_hooks_dir.join(suffix).to_string_lossy().as_ref(),
    ))
}

fn is_static_hook_path_suffix(suffix: &str) -> bool {
    !suffix.is_empty()
        && !suffix
            .chars()
            .any(|ch| matches!(ch, '\\' | '$' | '`' | '*' | '?' | '[' | '{' | '}'))
}

fn looks_like_windows_hook_command(command: &str) -> bool {
    let source_hooks_backslash_path = format!(
        r"{}\{EXTERNAL_AGENT_HOOKS_SUBDIR}\",
        external_agent_config_dir()
    );
    let project_dir_env_var = external_agent_project_dir_env_var();
    command.contains(&source_hooks_backslash_path)
        || command.contains(&format!("%{project_dir_env_var}%"))
        || command.contains(&format!("$env:{project_dir_env_var}"))
}

pub(super) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn copy_hook_scripts(
    source_external_agent_dir: &Path,
    target_config_dir: &Path,
) -> io::Result<()> {
    let source_hooks = source_external_agent_dir.join(EXTERNAL_AGENT_HOOKS_SUBDIR);
    if !source_hooks.is_dir() {
        return Ok(());
    }
    let target_hooks = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
    copy_dir_recursive_skip_existing(&source_hooks, &target_hooks)
}

fn copy_dir_recursive_skip_existing(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive_skip_existing(&source_path, &target_path)?;
        } else if file_type.is_file() && !target_path.exists() {
            fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

pub(crate) fn json_u64(value: &JsonValue) -> Option<u64> {
    if value.is_boolean() || value.is_null() {
        return None;
    }
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn is_missing_or_empty_text_file(path: &Path) -> io::Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    if !path.is_file() {
        return Ok(false);
    }

    Ok(fs::read_to_string(path)?.trim().is_empty())
}

pub(crate) fn external_agent_config_dir() -> String {
    format!(".{SOURCE_EXTERNAL_AGENT_NAME}")
}

pub(crate) fn external_agent_project_dir_env_var() -> String {
    format!(
        "{}_PROJECT_DIR",
        SOURCE_EXTERNAL_AGENT_NAME.to_ascii_uppercase()
    )
}
