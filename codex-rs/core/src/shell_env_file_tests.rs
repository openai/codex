use anyhow::Result;
use codex_protocol::ThreadId;
use codex_protocol::config_types::EnvironmentVariablePattern;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;

use super::*;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::empty_shell_snapshot_receiver;

#[cfg(not(windows))]
#[test]
fn shell_env_file_is_removed_when_session_owner_drops() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new(), ShellEnvCapture::Posix)?;
    let path = env_file.path().to_path_buf();

    assert!(path.exists());
    drop(env_file);
    assert!(!path.exists());

    Ok(())
}

#[cfg(not(windows))]
#[tokio::test]
async fn shell_env_file_captures_exports_without_exposing_writable_path() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new(), ShellEnvCapture::Posix)?;
    let base_env = HashMap::from([
        ("PATH".to_string(), "/usr/bin".to_string()),
        (
            CODEX_THREAD_ID_ENV_VAR.to_string(),
            "real-thread".to_string(),
        ),
        ("REMOVED_BY_HOOK".to_string(), "remove-me".to_string()),
    ]);
    std::fs::write(
        env_file.path(),
        "\
echo hidden
export CODEX_SESSION_START_TEST='from-session-start'
export PATH=\"/plugin/bin:$PATH\"
export COMMAND_SUBSTITUTION=$(printf unsafe)
export FUNCTION_DEF='() { echo unsafe; }'
export CODEX_ENV_FILE='/tmp/poison'
export CLAUDE_ENV_FILE='/tmp/poison'
export CODEX_THREAD_ID='poisoned-thread'
export EXPLICIT_OVERRIDE='from-hook'
unset REMOVED_BY_HOOK
",
    )?;
    let cwd = std::env::current_dir()?;
    env_file
        .capture_exports(&test_shell(), cwd.as_path(), &base_env)
        .await?;

    let mut env = base_env;
    env.insert(
        CODEX_ENV_FILE_ENV_VAR.to_string(),
        env_file.path().display().to_string(),
    );
    env.insert(
        CLAUDE_ENV_FILE_ENV_VAR.to_string(),
        env_file.path().display().to_string(),
    );
    let policy = ShellEnvironmentPolicy {
        r#set: HashMap::from([("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string())]),
        ..Default::default()
    };

    env_file.apply_exports(&mut env, &policy);

    assert_eq!(
        env,
        HashMap::from([
            ("PATH".to_string(), "/plugin/bin:/usr/bin".to_string()),
            (
                "CODEX_SESSION_START_TEST".to_string(),
                "from-session-start".to_string(),
            ),
            ("COMMAND_SUBSTITUTION".to_string(), "unsafe".to_string()),
            (
                "FUNCTION_DEF".to_string(),
                "() { echo unsafe; }".to_string(),
            ),
            (
                CODEX_THREAD_ID_ENV_VAR.to_string(),
                "real-thread".to_string(),
            ),
            ("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string()),
        ])
    );

    let mut snapshot_overrides = policy.r#set.clone();
    env_file.extend_snapshot_overrides(&mut snapshot_overrides, &env, &policy);
    assert_eq!(
        snapshot_overrides,
        HashMap::from([
            ("PATH".to_string(), "/plugin/bin:/usr/bin".to_string()),
            (
                "CODEX_SESSION_START_TEST".to_string(),
                "from-session-start".to_string(),
            ),
            ("COMMAND_SUBSTITUTION".to_string(), "unsafe".to_string()),
            (
                "FUNCTION_DEF".to_string(),
                "() { echo unsafe; }".to_string(),
            ),
            ("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string()),
            ("REMOVED_BY_HOOK".to_string(), String::new()),
        ])
    );

    Ok(())
}

#[cfg(not(windows))]
#[tokio::test]
async fn shell_env_file_filters_captured_exports_before_applying() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new(), ShellEnvCapture::Posix)?;
    let base_env = HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]);
    std::fs::write(
        env_file.path(),
        "\
export PATH=\"/plugin/bin:$PATH\"
export ALLOWED_VALUE='allowed'
export OPENAI_API_KEY='secret'
export BLOCKED_VALUE='blocked'
export NOT_INCLUDED='not-included'
export EXPLICIT_OVERRIDE='from-hook'
",
    )?;
    let cwd = std::env::current_dir()?;
    env_file
        .capture_exports(&test_shell(), cwd.as_path(), &base_env)
        .await?;

    let mut env = base_env;
    let policy = ShellEnvironmentPolicy {
        ignore_default_excludes: false,
        exclude: vec![EnvironmentVariablePattern::new_case_insensitive(
            "BLOCKED_*",
        )],
        include_only: vec![
            EnvironmentVariablePattern::new_case_insensitive("PATH"),
            EnvironmentVariablePattern::new_case_insensitive("ALLOWED_*"),
            EnvironmentVariablePattern::new_case_insensitive("EXPLICIT_*"),
        ],
        r#set: HashMap::from([("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string())]),
        ..Default::default()
    };

    env_file.apply_exports(&mut env, &policy);
    assert_eq!(
        env,
        HashMap::from([
            ("PATH".to_string(), "/plugin/bin:/usr/bin".to_string()),
            ("ALLOWED_VALUE".to_string(), "allowed".to_string()),
            ("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string()),
        ])
    );

    Ok(())
}

#[cfg(not(windows))]
fn test_shell() -> Shell {
    Shell {
        shell_type: ShellType::Sh,
        shell_path: PathBuf::from("/bin/sh"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    }
}

#[test]
fn shell_env_file_for_session_uses_powershell_script_suffix() -> Result<()> {
    let shell = test_powershell_shell();
    let env_file = ShellEnvFile::for_session(ThreadId::new(), &shell)?
        .expect("PowerShell should support a session env file");

    assert_eq!(
        env_file.path().extension().and_then(|ext| ext.to_str()),
        Some("ps1")
    );

    Ok(())
}

#[test]
fn shell_env_file_for_session_uses_cmd_script_suffix() -> Result<()> {
    let shell = test_cmd_shell();
    let env_file = ShellEnvFile::for_session(ThreadId::new(), &shell)?
        .expect("cmd should support a session env file");

    assert_eq!(
        env_file.path().extension().and_then(|ext| ext.to_str()),
        Some("cmd")
    );

    Ok(())
}

#[test]
fn powershell_env_output_parser_accepts_json_object() -> Result<()> {
    assert_eq!(
        parse_powershell_env_output(br#"{"ONLY":"one","EMPTY":""}"#)?,
        HashMap::from([
            ("ONLY".to_string(), "one".to_string()),
            ("EMPTY".to_string(), "".to_string()),
        ])
    );

    Ok(())
}

#[test]
fn cmd_env_output_parser_accepts_set_output() {
    assert_eq!(
        parse_cmd_env_output(b"=C:=C:\\worktree\r\nEMPTY=\r\nONLY=one\r\nWITH_EQUALS=a=b\r\n"),
        HashMap::from([
            ("EMPTY".to_string(), "".to_string()),
            ("ONLY".to_string(), "one".to_string()),
            ("WITH_EQUALS".to_string(), "a=b".to_string()),
        ])
    );
}

#[cfg(windows)]
#[tokio::test]
async fn powershell_env_file_applies_exports_without_exposing_writable_path() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new(), ShellEnvCapture::PowerShell)?;
    let base_env = HashMap::from([
        ("BASE".to_string(), "keep".to_string()),
        (
            CODEX_THREAD_ID_ENV_VAR.to_string(),
            "real-thread".to_string(),
        ),
        ("REMOVED_BY_HOOK".to_string(), "remove-me".to_string()),
    ]);
    std::fs::write(
        env_file.path(),
        r#"
Write-Output "hidden"
$env:CODEX_SESSION_START_TEST = "from-session-start"
$env:COMMAND_EXPRESSION = ("unsafe").ToUpperInvariant()
$env:CODEX_ENV_FILE = "C:\poison"
$env:CLAUDE_ENV_FILE = "C:\poison"
$env:CODEX_THREAD_ID = "poisoned-thread"
$env:EXPLICIT_OVERRIDE = "from-hook"
Remove-Item Env:REMOVED_BY_HOOK -ErrorAction SilentlyContinue
"#,
    )?;
    let cwd = std::env::current_dir()?;
    env_file
        .capture_exports(&test_powershell_shell(), cwd.as_path(), &base_env)
        .await?;

    let mut env = base_env;
    env.insert(
        CODEX_ENV_FILE_ENV_VAR.to_string(),
        env_file.path().display().to_string(),
    );
    env.insert(
        CLAUDE_ENV_FILE_ENV_VAR.to_string(),
        env_file.path().display().to_string(),
    );
    let policy = ShellEnvironmentPolicy {
        r#set: HashMap::from([("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string())]),
        ..Default::default()
    };

    env_file.apply_exports(&mut env, &policy);

    assert_eq!(
        env,
        HashMap::from([
            ("BASE".to_string(), "keep".to_string()),
            (
                "CODEX_SESSION_START_TEST".to_string(),
                "from-session-start".to_string(),
            ),
            ("COMMAND_EXPRESSION".to_string(), "UNSAFE".to_string()),
            (
                CODEX_THREAD_ID_ENV_VAR.to_string(),
                "real-thread".to_string(),
            ),
            ("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string()),
        ])
    );

    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn cmd_env_file_applies_exports_without_exposing_writable_path() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new(), ShellEnvCapture::Cmd)?;
    let base_env = HashMap::from([
        ("BASE".to_string(), "keep".to_string()),
        (
            CODEX_THREAD_ID_ENV_VAR.to_string(),
            "real-thread".to_string(),
        ),
        ("REMOVED_BY_HOOK".to_string(), "remove-me".to_string()),
    ]);
    std::fs::write(
        env_file.path(),
        "\
@echo off\r
set CODEX_SESSION_START_TEST=from-session-start\r
set CODEX_ENV_FILE=C:\\poison\r
set CLAUDE_ENV_FILE=C:\\poison\r
set CODEX_THREAD_ID=poisoned-thread\r
set EXPLICIT_OVERRIDE=from-hook\r
set REMOVED_BY_HOOK=\r
",
    )?;
    let cwd = std::env::current_dir()?;
    env_file
        .capture_exports(&test_cmd_shell(), cwd.as_path(), &base_env)
        .await?;

    let mut env = base_env;
    env.insert(
        CODEX_ENV_FILE_ENV_VAR.to_string(),
        env_file.path().display().to_string(),
    );
    env.insert(
        CLAUDE_ENV_FILE_ENV_VAR.to_string(),
        env_file.path().display().to_string(),
    );
    let policy = ShellEnvironmentPolicy {
        r#set: HashMap::from([("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string())]),
        ..Default::default()
    };

    env_file.apply_exports(&mut env, &policy);

    assert_eq!(
        env,
        HashMap::from([
            ("BASE".to_string(), "keep".to_string()),
            (
                "CODEX_SESSION_START_TEST".to_string(),
                "from-session-start".to_string(),
            ),
            (
                CODEX_THREAD_ID_ENV_VAR.to_string(),
                "real-thread".to_string(),
            ),
            ("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string()),
        ])
    );

    Ok(())
}

fn test_powershell_shell() -> Shell {
    Shell {
        shell_type: ShellType::PowerShell,
        shell_path: PathBuf::from("powershell.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    }
}

fn test_cmd_shell() -> Shell {
    Shell {
        shell_type: ShellType::Cmd,
        shell_path: PathBuf::from("cmd.exe"),
        shell_snapshot: empty_shell_snapshot_receiver(),
    }
}
