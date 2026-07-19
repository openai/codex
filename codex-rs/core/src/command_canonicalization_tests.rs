use super::canonicalize_command_for_approval;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

fn powershell(executable: &str, command_flag: &str, script: &str) -> Vec<String> {
    vec![
        executable.to_string(),
        "-NoProfile".to_string(),
        command_flag.to_string(),
        script.to_string(),
    ]
}

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
fn preserves_exact_powershell_approval_identity() {
    let base = powershell("C:/workspace/powershell.exe", "-Command", "Write-Host hi");
    let variants = [
        powershell("powershell.exe", "-Command", "Write-Host hi"),
        powershell("C:/other/powershell.exe", "-Command", "Write-Host hi"),
        powershell("C:/workspace/pwsh.exe", "-Command", "Write-Host hi"),
        powershell("C:/workspace/powershell.exe", "-c", "Write-Host hi"),
        powershell(
            "C:/workspace/powershell.exe",
            "-Command",
            "Write-Host changed",
        ),
        powershell("C:/workspace/powershell.exe.", "-Command", "Write-Host hi"),
        powershell("C:/workspace/powershell.exe ", "-Command", "Write-Host hi"),
        powershell(
            r"\\?\C:\workspace\powershell.exe.",
            "-Command",
            "Write-Host hi",
        ),
        powershell(
            r"\\.\UNC\server\share\powershell.exe ",
            "-Command",
            "Write-Host hi",
        ),
    ];

    let mut commands = vec![base];
    commands.extend(variants);
    let keys: HashSet<_> = commands
        .iter()
        .map(|command| canonicalize_command_for_approval(command))
        .collect();

    assert_eq!(keys.len(), commands.len());
    for command in commands {
        assert_eq!(canonicalize_command_for_approval(&command), command);
    }
}

#[test]
fn powershell_identity_does_not_collide_with_raw_marker_argv() {
    let powershell = vec![
        "powershell.exe".to_string(),
        "-Command".to_string(),
        "Write-Host hi".to_string(),
    ];
    let mut marker_argv = vec!["__codex_powershell_script__".to_string()];
    marker_argv.extend(powershell.iter().cloned());

    assert_eq!(canonicalize_command_for_approval(&powershell), powershell);
    assert_eq!(canonicalize_command_for_approval(&marker_argv), marker_argv);
    assert_ne!(
        canonicalize_command_for_approval(&powershell),
        canonicalize_command_for_approval(&marker_argv)
    );
}

#[test]
fn preserves_non_shell_commands() {
    let command = vec!["cargo".to_string(), "fmt".to_string()];
    assert_eq!(canonicalize_command_for_approval(&command), command);
}
