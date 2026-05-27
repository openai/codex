use anyhow::Result;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use tempfile::tempdir;

use super::*;

#[cfg(not(windows))]
#[test]
fn shell_env_file_is_removed_when_session_owner_drops() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = AbsolutePathBuf::from_absolute_path(dir.path())?;
    let env_file = ShellEnvFile::new(&codex_home, ThreadId::new())?;
    let path = env_file.path().to_path_buf();

    assert!(path.exists());
    drop(env_file);
    assert!(!path.exists());

    Ok(())
}
