use anyhow::Result;
use codex_protocol::ThreadId;
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
    let env_file = ShellEnvFile::new(ThreadId::new())?;
    let path = env_file.path().to_path_buf();

    assert!(path.exists());
    drop(env_file);
    assert!(!path.exists());

    Ok(())
}

#[cfg(not(windows))]
#[tokio::test]
async fn shell_env_file_applies_exports_without_exposing_writable_path() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new())?;
    let base_env = HashMap::from([
        ("PATH".to_string(), "/usr/bin".to_string()),
        (
            CODEX_THREAD_ID_ENV_VAR.to_string(),
            "real-thread".to_string(),
        ),
    ]);
    std::fs::write(
        env_file.path(),
        "\
export CODEX_SESSION_START_TEST='from-session-start'
export PATH=\"/plugin/bin:$PATH\"
export CODEX_ENV_FILE='/tmp/poison'
export CODEX_THREAD_ID='poisoned-thread'
export EXPLICIT_OVERRIDE='from-hook'
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
    let explicit_env_overrides =
        HashMap::from([("EXPLICIT_OVERRIDE".to_string(), "from-policy".to_string())]);

    env_file.apply_exports(&mut env, &explicit_env_overrides);

    assert_eq!(
        env,
        HashMap::from([
            ("PATH".to_string(), "/plugin/bin:/usr/bin".to_string()),
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

#[cfg(not(windows))]
#[tokio::test]
async fn shell_env_file_sources_shell_code_once() -> Result<()> {
    let env_file = ShellEnvFile::new(ThreadId::new())?;
    std::fs::write(
        env_file.path(),
        "\
echo hidden
export SAFE=value
export COMMAND_SUBSTITUTION=$(printf unsafe)
export FUNCTION_DEF='() { echo unsafe; }'
",
    )?;
    let cwd = std::env::current_dir()?;
    env_file
        .capture_exports(&test_shell(), cwd.as_path(), &HashMap::new())
        .await?;

    let mut env = HashMap::new();
    env_file.apply_exports(&mut env, &HashMap::new());

    assert_eq!(
        env,
        HashMap::from([
            ("SAFE".to_string(), "value".to_string()),
            ("COMMAND_SUBSTITUTION".to_string(), "unsafe".to_string()),
            (
                "FUNCTION_DEF".to_string(),
                "() { echo unsafe; }".to_string(),
            ),
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
