use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

pub(crate) fn replace_last_codex_command(new_command: &str) -> io::Result<()> {
    let Some(history_path) = resolve_history_path() else {
        return Ok(());
    };
    let contents = match fs::read_to_string(&history_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    let Some(updated) = update_history_contents(&contents, new_command) else {
        return Ok(());
    };
    fs::write(history_path, updated)
}

fn resolve_history_path() -> Option<PathBuf> {
    if let Some(history_file) = env::var_os("HISTFILE")
        && !history_file.is_empty()
    {
        return Some(PathBuf::from(history_file));
    }
    let home = PathBuf::from(env::var_os("HOME")?);
    let shell = env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        return Some(home.join(".zsh_history"));
    }
    if shell.contains("fish") {
        let data_home = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));
        return Some(data_home.join("fish").join("fish_history"));
    }
    if shell.contains("bash") {
        return Some(home.join(".bash_history"));
    }
    Some(home.join(".bash_history"))
}

fn update_history_contents(contents: &str, new_command: &str) -> Option<String> {
    let mut lines: Vec<String> = contents.split('\n').map(str::to_string).collect();
    if replace_last_codex_line(&mut lines, new_command) {
        Some(lines.join("\n"))
    } else {
        None
    }
}

fn replace_last_codex_line(lines: &mut [String], new_command: &str) -> bool {
    for line in lines.iter_mut().rev() {
        if let Some(updated) = replace_zsh_history_line(line, new_command)
            .or_else(|| replace_fish_history_line(line, new_command))
            .or_else(|| replace_plain_history_line(line, new_command))
        {
            *line = updated;
            return true;
        }
    }
    false
}

fn replace_zsh_history_line(line: &str, new_command: &str) -> Option<String> {
    if !line.starts_with(": ") {
        return None;
    }
    let semicolon_index = line.find(';')?;
    let (prefix, command) = line.split_at(semicolon_index + 1);
    if !is_codex_command(command) {
        return None;
    }
    let leading_len = command.len() - command.trim_start().len();
    let leading = &command[..leading_len];
    Some(format!("{prefix}{leading}{new_command}"))
}

fn replace_fish_history_line(line: &str, new_command: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let prefix_len = line.len() - trimmed.len();
    let prefix = &line[..prefix_len];
    let (cmd_prefix, rest) = if let Some(rest) = trimmed.strip_prefix("- cmd: ") {
        ("- cmd: ", rest)
    } else if let Some(rest) = trimmed.strip_prefix("cmd: ") {
        ("cmd: ", rest)
    } else {
        return None;
    };
    let rest = rest.trim_start();
    let is_quoted = rest.starts_with('"');
    let rest = rest.trim_start_matches('"');
    if !is_codex_command(rest) {
        return None;
    }
    let quote = if is_quoted { "\"" } else { "" };
    Some(format!("{prefix}{cmd_prefix}{quote}{new_command}{quote}"))
}

fn replace_plain_history_line(line: &str, new_command: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !is_codex_command(trimmed) {
        return None;
    }
    let prefix_len = line.len() - trimmed.len();
    let prefix = &line[..prefix_len];
    Some(format!("{prefix}{new_command}"))
}

fn is_codex_command(command: &str) -> bool {
    let trimmed = command.trim_start();
    trimmed == "codex" || trimmed.starts_with("codex ")
}
