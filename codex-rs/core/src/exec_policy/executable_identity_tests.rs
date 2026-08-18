use super::shell_approval_command;
use crate::shell::Shell;
use crate::shell::ShellType;
use codex_tools::UnifiedExecShellMode;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn parent_directory_traversal_is_not_a_trusted_system_shell() {
    let (configured_executable, unfamiliar_executable) = if cfg!(windows) {
        let system_root = std::env::var_os("SystemRoot").expect("Windows SystemRoot");
        let system_directory = PathBuf::from(system_root).join("System32");
        (
            system_directory.join("cmd.exe"),
            system_directory
                .join("..")
                .join("workspace")
                .join("powershell.exe"),
        )
    } else {
        (
            PathBuf::from("/bin/sh"),
            PathBuf::from("/bin/../workspace/bash"),
        )
    };
    let shell = Shell {
        shell_type: if cfg!(windows) {
            ShellType::Cmd
        } else {
            ShellType::Sh
        },
        shell_path: configured_executable,
    };
    let command = vec![
        unfamiliar_executable.to_string_lossy().into_owned(),
        "-c".to_string(),
        "ls".to_string(),
    ];

    assert_eq!(
        shell_approval_command(&command, &shell, &UnifiedExecShellMode::Direct),
        &command[..1],
    );
}

#[cfg(windows)]
#[test]
fn windows_shell_identity_is_case_insensitive() {
    let configured_executable = PathBuf::from(r"C:\Custom\pwsh.exe");
    let configured_shell = Shell {
        shell_type: ShellType::PowerShell,
        shell_path: configured_executable.clone(),
    };
    let system_root = std::env::var_os("SystemRoot").expect("Windows SystemRoot");
    let system_executable = PathBuf::from(system_root.to_string_lossy().to_ascii_uppercase())
        .join("SYSTEM32")
        .join("WINDOWSPOWERSHELL")
        .join("V1.0")
        .join("POWERSHELL.EXE");

    for executable in [
        configured_executable.to_string_lossy().to_ascii_uppercase(),
        system_executable.to_string_lossy().into_owned(),
    ] {
        let command = vec![executable, "-Command".to_string(), "echo safe".to_string()];

        assert_eq!(
            shell_approval_command(&command, &configured_shell, &UnifiedExecShellMode::Direct),
            command.as_slice(),
        );
    }
}
