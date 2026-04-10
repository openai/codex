use super::*;
use std::path::PathBuf;
use std::process::Command;

#[test]
#[cfg(target_os = "macos")]
fn detects_zsh() {
    let zsh_shell = get_shell(ShellType::Zsh, /*path*/ None).unwrap();

    let shell_path = zsh_shell.shell_path;

    assert_eq!(shell_path, std::path::Path::new("/bin/zsh"));
}

#[test]
#[cfg(target_os = "macos")]
fn fish_fallback_to_zsh() {
    let zsh_shell = default_user_shell_from_path(Some(PathBuf::from("/bin/fish")));

    let shell_path = zsh_shell.shell_path;

    assert_eq!(shell_path, std::path::Path::new("/bin/zsh"));
}

#[test]
fn detects_bash() {
    let bash_shell = get_shell(ShellType::Bash, /*path*/ None).unwrap();
    let shell_path = bash_shell.shell_path;

    assert!(
        shell_path.file_name().and_then(|name| name.to_str()) == Some("bash"),
        "shell path: {shell_path:?}",
    );
}

#[test]
fn detects_sh() {
    let sh_shell = get_shell(ShellType::Sh, /*path*/ None).unwrap();
    let shell_path = sh_shell.shell_path;
    assert!(
        shell_path.file_name().and_then(|name| name.to_str()) == Some("sh"),
        "shell path: {shell_path:?}",
    );
}

#[test]
fn can_run_on_shell_test() {
    let cmd = "echo \"Works\"";
    if cfg!(windows) {
        assert!(shell_works(
            get_shell(ShellType::PowerShell, /*path*/ None),
            "Out-String 'Works'",
            /*required*/ true,
        ));
        assert!(shell_works(
            get_shell(ShellType::Cmd, /*path*/ None),
            cmd,
            /*required*/ true,
        ));
        assert!(shell_works(
            Some(ultimate_fallback_shell()),
            cmd,
            /*required*/ true
        ));
    } else {
        assert!(shell_works(
            Some(ultimate_fallback_shell()),
            cmd,
            /*required*/ true
        ));
        assert!(shell_works(
            get_shell(ShellType::Zsh, /*path*/ None),
            cmd,
            /*required*/ false
        ));
        assert!(shell_works(
            get_shell(ShellType::Bash, /*path*/ None),
            cmd,
            /*required*/ true
        ));
        assert!(shell_works(
            get_shell(ShellType::Sh, /*path*/ None),
            cmd,
            /*required*/ true
        ));
    }
}

#[test]
fn windows_default_shell_prefers_healthy_powershell() {
    let powershell = Shell {
        shell_type: ShellType::PowerShell,
        shell_path: PathBuf::from("pwsh.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };
    let cmd = Shell {
        shell_type: ShellType::Cmd,
        shell_path: PathBuf::from("cmd.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };

    assert_eq!(
        default_windows_shell(Some(powershell), Some(cmd), |_| true).shell_type,
        ShellType::PowerShell
    );
}

#[test]
fn windows_default_shell_falls_back_to_cmd_when_powershell_fails_probe() {
    let powershell = Shell {
        shell_type: ShellType::PowerShell,
        shell_path: PathBuf::from("pwsh.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };
    let cmd = Shell {
        shell_type: ShellType::Cmd,
        shell_path: PathBuf::from("cmd.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };

    assert_eq!(
        default_windows_shell(Some(powershell), Some(cmd), |_| false).shell_type,
        ShellType::Cmd
    );
}

fn shell_works(shell: Option<Shell>, command: &str, required: bool) -> bool {
    if let Some(shell) = shell {
        let args = shell.derive_exec_args(command, /*use_login_shell*/ false);
        let output = Command::new(args[0].clone())
            .args(&args[1..])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Works"));
        true
    } else {
        !required
    }
}

#[test]
fn derive_exec_args() {
    let test_bash_shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };
    assert_eq!(
        test_bash_shell.derive_exec_args("echo hello", /*use_login_shell*/ false),
        vec!["/bin/bash", "-c", "echo hello"]
    );
    assert_eq!(
        test_bash_shell.derive_exec_args("echo hello", /*use_login_shell*/ true),
        vec!["/bin/bash", "-lc", "echo hello"]
    );

    let test_zsh_shell = Shell {
        shell_type: ShellType::Zsh,
        shell_path: PathBuf::from("/bin/zsh"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };
    assert_eq!(
        test_zsh_shell.derive_exec_args("echo hello", /*use_login_shell*/ false),
        vec!["/bin/zsh", "-c", "echo hello"]
    );
    assert_eq!(
        test_zsh_shell.derive_exec_args("echo hello", /*use_login_shell*/ true),
        vec!["/bin/zsh", "-lc", "echo hello"]
    );

    let test_powershell_shell = Shell {
        shell_type: ShellType::PowerShell,
        shell_path: PathBuf::from("pwsh.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    };
    assert_eq!(
        test_powershell_shell.derive_exec_args("echo hello", /*use_login_shell*/ false),
        vec!["pwsh.exe", "-NoProfile", "-Command", "echo hello"]
    );
    assert_eq!(
        test_powershell_shell.derive_exec_args("echo hello", /*use_login_shell*/ true),
        vec!["pwsh.exe", "-Command", "echo hello"]
    );
}

#[tokio::test]
async fn test_current_shell_detects_zsh() {
    let shell = Command::new("sh")
        .arg("-c")
        .arg("echo $SHELL")
        .output()
        .unwrap();

    let shell_path = String::from_utf8_lossy(&shell.stdout).trim().to_string();
    if shell_path.ends_with("/zsh") {
        assert_eq!(
            default_user_shell(),
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: PathBuf::from(shell_path),
                shell_snapshot: empty_shell_snapshot_receiver(),
            }
        );
    }
}

#[tokio::test]
async fn detects_powershell_as_default() {
    if !cfg!(windows) {
        return;
    }

    let default_shell = default_user_shell();

    match default_shell.shell_type {
        ShellType::PowerShell => {
            let shell_path = default_shell.shell_path;
            assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
        }
        ShellType::Cmd => {
            assert!(
                !get_shell(ShellType::PowerShell, /*path*/ None)
                    .is_some_and(|shell| shell_starts_successfully(&shell))
            );
        }
        other => panic!("unexpected Windows default shell: {other:?}"),
    }
}

#[test]
fn finds_powershell() {
    if !cfg!(windows) {
        return;
    }

    let powershell_shell = get_shell(ShellType::PowerShell, /*path*/ None).unwrap();
    let shell_path = powershell_shell.shell_path;

    assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
}
