use codex_shell_command::bash::extract_bash_command;
use codex_shell_command::bash::parse_shell_lc_plain_commands;

const CANONICAL_BASH_SCRIPT_PREFIX: &str = "__codex_shell_script__";

/// Canonicalize command argv for approval-cache matching.
///
/// Bash word-only scripts retain their historical inner-command identity. For
/// PowerShell, the executable, wrapper flags, and exact script are all part of
/// the key because each affects the authorization boundary.
pub(crate) fn canonicalize_command_for_approval(command: &[String]) -> Vec<String> {
    if let Some(commands) = parse_shell_lc_plain_commands(command)
        && let [single_command] = commands.as_slice()
    {
        return single_command.clone();
    }

    if let Some((_shell, script)) = extract_bash_command(command) {
        let shell_mode = command.get(1).cloned().unwrap_or_default();
        return vec![
            CANONICAL_BASH_SCRIPT_PREFIX.to_string(),
            shell_mode,
            script.to_string(),
        ];
    }

    command.to_vec()
}

#[cfg(test)]
#[path = "command_canonicalization_tests.rs"]
mod tests;
