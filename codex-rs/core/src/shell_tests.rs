use super::*;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;

#[test]
#[cfg(target_os = "macos")]
fn detects_zsh() {
    let zsh_shell = get_shell(ShellType::Zsh).unwrap();

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
    let bash_shell = get_shell(ShellType::Bash).unwrap();
    let shell_path = bash_shell.shell_path;

    assert!(
        shell_path.file_name().and_then(|name| name.to_str()) == Some("bash"),
        "shell path: {shell_path:?}",
    );
}

#[test]
fn detects_sh() {
    let sh_shell = get_shell(ShellType::Sh).unwrap();
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
            get_shell(ShellType::PowerShell),
            "Out-String 'Works'",
            /*required*/ true,
        ));
        assert!(shell_works(
            get_shell(ShellType::Cmd),
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
            get_shell(ShellType::Zsh),
            cmd,
            /*required*/ false
        ));
        assert!(shell_works(
            get_shell(ShellType::Bash),
            cmd,
            /*required*/ true
        ));
        assert!(shell_works(
            get_shell(ShellType::Sh),
            cmd,
            /*required*/ true
        ));
    }
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

/// Diagnostics and `$LINENO` must refer to the original script, even with newlines in the package path.
#[test]
fn codex_path_setup_preserves_shell_script_line_numbers() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let run = |argv: &[String]| {
        Command::new(&argv[0])
            .args(&argv[1..])
            .env("ZDOTDIR", temp_dir.path())
            .env("HOME", temp_dir.path())
            .env_remove("BASH_ENV")
            .output()
    };
    for shell_type in [ShellType::Bash, ShellType::Zsh, ShellType::Sh] {
        let Some(shell) = get_shell(shell_type) else {
            assert_eq!(shell_type, ShellType::Zsh, "only zsh may be unavailable");
            continue;
        };
        let invocation = ShellInvocation {
            shell,
            use_login_shell: true,
        };
        for directory in [
            "codex-path",
            "codex '\\c[*]$(printf bad)`printf bad`\npath\n",
        ] {
            let directory = temp_dir.path().join(directory);
            let path = PathUri::from_host_native_path(&directory)?;
            let commands = [
                ":\nif then",
                "printf '%s\\n' \"$LINENO\"\nprintf '%s\\n' \"$LINENO\"",
            ];
            for command in commands {
                if command.contains("LINENO") && shell_type == ShellType::Sh {
                    continue; // dash does not implement LINENO.
                }
                let original = invocation
                    .shell
                    .derive_exec_args(command, /*use_login_shell*/ true);
                let wrapped = invocation
                    .derive_exec_args_with_path_prepends(command, std::slice::from_ref(&path))
                    .expect("a POSIX package path should be usable");
                assert_eq!(
                    run(&wrapped)?,
                    run(&original)?,
                    "{shell_type:?}: {directory:?}"
                );
                assert_eq!(wrapped[2].lines().count(), command.lines().count());
            }

            let command = "printf '%s' \"$PATH\"";
            let wrapped = invocation
                .derive_exec_args_with_path_prepends(command, std::slice::from_ref(&path))
                .expect("a POSIX package path should be usable");
            let output = run(&wrapped)?;
            assert!(output.status.success());
            let actual_path = String::from_utf8(output.stdout)?;
            assert_eq!(actual_path.split(':').next(), directory.to_str());
        }
    }
    Ok(())
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
            }
        );
    }
}

#[tokio::test]
async fn detects_powershell_as_default() {
    if !cfg!(windows) {
        return;
    }

    let powershell_shell = default_user_shell();
    let shell_path = powershell_shell.shell_path;

    assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
}

#[test]
fn finds_powershell() {
    if !cfg!(windows) {
        return;
    }

    let powershell_shell = get_shell(ShellType::PowerShell).unwrap();
    let shell_path = powershell_shell.shell_path;

    assert!(shell_path.ends_with("pwsh.exe") || shell_path.ends_with("powershell.exe"));
}
