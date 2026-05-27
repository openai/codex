use anyhow::Result;
use codex_protocol::ThreadId;

use super::*;

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
