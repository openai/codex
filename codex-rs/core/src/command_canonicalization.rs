use crate::bash::extract_bash_command;
use crate::bash::parse_shell_lc_plain_commands;
use crate::powershell::extract_powershell_command;

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
mod tests {
    use super::canonicalize_command_for_approval;
    use pretty_assertions::assert_eq;

    #[test]
    fn canonicalizes_word_only_shell_scripts_to_inner_command() {
        let command_a = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "cargo test -p codex-core".to_string(),
        ];
        let command_b = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cargo   test   -p codex-core".to_string(),
        ];

        assert_eq!(
            canonicalize_command_for_approval(&command_a),
            vec![
                "cargo".to_string(),
                "test".to_string(),
                "-p".to_string(),
                "codex-core".to_string(),
            ]
        );
        assert_eq!(
            canonicalize_command_for_approval(&command_a),
            canonicalize_command_for_approval(&command_b)
        );
    }

    #[test]
    fn canonicalizes_heredoc_scripts_to_stable_script_key() {
        let script = "python3 <<'PY'\nprint('hello')\nPY";
        let command_a = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            script.to_string(),
        ];
        let command_b = vec!["zsh".to_string(), "-lc".to_string(), script.to_string()];

        assert_eq!(
            canonicalize_command_for_approval(&command_a),
            vec![
                "__codex_shell_script__".to_string(),
                "-lc".to_string(),
                script.to_string(),
            ]
        );
        assert_eq!(
            canonicalize_command_for_approval(&command_a),
            canonicalize_command_for_approval(&command_b)
        );
    }

    #[test]
    fn canonicalizes_powershell_wrappers_to_stable_script_key() {
        let script = "Write-Host hi";
        let command_a = vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ];
        let command_b = vec![
            "powershell".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ];

        assert_eq!(
            canonicalize_command_for_approval(&command_a),
            vec![
                "__codex_powershell_script__".to_string(),
                script.to_string(),
            ]
        );
        assert_eq!(
            canonicalize_command_for_approval(&command_a),
            canonicalize_command_for_approval(&command_b)
        );
    }

    #[test]
    fn preserves_non_shell_commands() {
        let command = vec!["cargo".to_string(), "fmt".to_string()];
        assert_eq!(canonicalize_command_for_approval(&command), command);
    }
}
