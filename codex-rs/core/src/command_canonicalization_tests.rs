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

#[test]
fn preserves_shell_like_paths_in_approval_keys() {
    let command = vec![
        ".poc/bash".to_string(),
        "-lc".to_string(),
        "cargo test -p codex-core".to_string(),
    ];

    assert_eq!(canonicalize_command_for_approval(&command), command);
}
