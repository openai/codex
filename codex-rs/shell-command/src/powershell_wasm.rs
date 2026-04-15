use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::shell_detect::ShellType;
use crate::shell_detect::detect_shell_type;

const POWERSHELL_FLAGS: &[&str] = &["-nologo", "-noprofile", "-command", "-c"];

pub const UTF8_OUTPUT_PREFIX: &str = "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\n";

pub fn prefix_powershell_script_with_utf8(command: &[String]) -> Vec<String> {
    let Some((_, script)) = extract_powershell_command(command) else {
        return command.to_vec();
    };

    let trimmed = script.trim_start();
    let script = if trimmed.starts_with(UTF8_OUTPUT_PREFIX) {
        script.to_string()
    } else {
        format!("{UTF8_OUTPUT_PREFIX}{script}")
    };

    let mut command: Vec<String> = command[..(command.len() - 1)]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    command.push(script);
    command
}

pub fn extract_powershell_command(command: &[String]) -> Option<(&str, &str)> {
    if command.len() < 3 {
        return None;
    }

    let shell = &command[0];
    if !matches!(
        detect_shell_type(&PathBuf::from(shell)),
        Some(ShellType::PowerShell)
    ) {
        return None;
    }

    let mut i = 1usize;
    while i + 1 < command.len() {
        let flag = &command[i];
        if !POWERSHELL_FLAGS.contains(&flag.to_ascii_lowercase().as_str()) {
            return None;
        }
        if flag.eq_ignore_ascii_case("-Command") || flag.eq_ignore_ascii_case("-c") {
            let script = &command[i + 1];
            return Some((shell, script));
        }
        i += 1;
    }
    None
}

pub(crate) fn try_find_powershellish_executable_blocking() -> Option<AbsolutePathBuf> {
    None
}

pub fn try_find_powershell_executable_blocking() -> Option<AbsolutePathBuf> {
    None
}

pub fn try_find_pwsh_executable_blocking() -> Option<AbsolutePathBuf> {
    None
}
