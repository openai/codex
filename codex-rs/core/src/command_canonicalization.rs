use codex_shell_command::bash::extract_bash_command;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::powershell::extract_powershell_command;

const CANONICAL_BASH_SCRIPT_PREFIX: &str = "__codex_shell_script__";
const CANONICAL_POWERSHELL_SCRIPT_PREFIX: &str = "__codex_powershell_script__";

/// Canonicalize command argv for approval-cache matching.
///
/// This keeps approval decisions stable across wrapper-path differences (for
/// example `/bin/bash -lc` vs `bash -lc`) and across shell wrapper tools while
/// preserving exact script text for complex scripts where we cannot safely
/// recover a tokenized command sequence.
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

    if let Some((_shell, script)) = extract_powershell_command(command) {
        return vec![
            CANONICAL_POWERSHELL_SCRIPT_PREFIX.to_string(),
            script.to_string(),
        ];
    }

    command.to_vec()
}

#[cfg(test)]
#[path = "command_canonicalization_tests.rs"]
mod tests;
