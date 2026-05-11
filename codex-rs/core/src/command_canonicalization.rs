use codex_shell_command::bash::extract_bash_command;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::powershell::extract_powershell_command;

const CANONICAL_BASH_SCRIPT_PREFIX: &str = "__codex_shell_script__";
const CANONICAL_POWERSHELL_SCRIPT_PREFIX: &str = "__codex_powershell_script__";

/// Canonicalize command argv for approval-cache matching.
///
/// This keeps approval decisions stable across shell script whitespace changes
/// while preserving the shell executable identity. The executable is part of the
/// key because it starts before the shell script and can be attacker controlled.
pub(crate) fn canonicalize_command_for_approval(command: &[String]) -> Vec<String> {
    if let Some((shell, script)) = extract_bash_command(command) {
        let shell_mode = command.get(1).cloned().unwrap_or_default();
        if let Some(commands) = parse_shell_lc_plain_commands(command)
            && let [single_command] = commands.as_slice()
        {
            let mut canonical_command = vec![shell.to_string(), shell_mode];
            canonical_command.extend(single_command.clone());
            return canonical_command;
        }

        return vec![
            CANONICAL_BASH_SCRIPT_PREFIX.to_string(),
            shell.to_string(),
            shell_mode,
            script.to_string(),
        ];
    }

    if let Some((shell, script)) = extract_powershell_command(command) {
        return vec![
            CANONICAL_POWERSHELL_SCRIPT_PREFIX.to_string(),
            shell.to_string(),
            script.to_string(),
        ];
    }

    command.to_vec()
}

#[cfg(test)]
#[path = "command_canonicalization_tests.rs"]
mod tests;
