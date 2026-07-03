use super::canonicalize_command_for_approval;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

fn posix(shell: &str, mode: &str, script: &str) -> Vec<String> {
    vec![shell.to_string(), mode.to_string(), script.to_string()]
}

fn powershell(executable: &str, command_flag: &str, script: &str) -> Vec<String> {
    vec![
        executable.to_string(),
        "-NoProfile".to_string(),
        command_flag.to_string(),
        script.to_string(),
    ]
}

#[test]
fn preserves_exact_posix_approval_identity() {
    let base = posix("/bin/bash", "-lc", "printf '%s\\n' value");
    let commands = vec![
        base.clone(),
        posix("bash", "-lc", "printf '%s\\n' value"),
        posix("./bash", "-lc", "printf '%s\\n' value"),
        posix("/tmp/workspace/bash", "-lc", "printf '%s\\n' value"),
        posix("/bin/zsh", "-lc", "printf '%s\\n' value"),
        posix("/bin/sh", "-lc", "printf '%s\\n' value"),
        posix("/bin/bash", "-c", "printf '%s\\n' value"),
        posix("/bin/bash", "-lc", "printf  '%s\\n'  value"),
        posix("/bin/bash", "-lc", "printf \"%s\\n\" value"),
        posix("/bin/bash", "-lc", "printf '%s\\n' *"),
        posix("/bin/bash", "-lc", "printf '%s\\n' ~"),
        posix("/bin/bash", "-lc", "printf '%s\\n' \"$(id)\""),
        posix("/bin/bash", "-lc", "printf '%s\\n' value >out"),
        posix("/bin/bash", "-lc", "python3 <<'PY'\nprint('value')\nPY"),
        posix("/bin/bash", "-lc", ""),
        posix("/bin/bash", "-lc", "printf '%s\\n' changed"),
        vec![
            "printf".to_string(),
            "%s\\n".to_string(),
            "value".to_string(),
        ],
    ];

    let keys: HashSet<_> = commands
        .iter()
        .map(|command| canonicalize_command_for_approval(command))
        .collect();

    assert_eq!(keys.len(), commands.len());
    assert_eq!(
        canonicalize_command_for_approval(&base),
        canonicalize_command_for_approval(&base)
    );
    for command in commands {
        assert_eq!(canonicalize_command_for_approval(&command), command);
    }
}

#[test]
fn posix_identity_does_not_collide_with_raw_marker_argv() {
    let shell = posix("/bin/bash", "-lc", "echo marker");
    let mut marker_argv = vec!["__codex_shell_script__".to_string()];
    marker_argv.extend(shell.iter().cloned());

    assert_eq!(canonicalize_command_for_approval(&shell), shell);
    assert_eq!(canonicalize_command_for_approval(&marker_argv), marker_argv);
    assert_ne!(
        canonicalize_command_for_approval(&shell),
        canonicalize_command_for_approval(&marker_argv)
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
