#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use std::io::BufRead;
use std::io::Read;
use std::process::Stdio;

#[test]
fn deleting_absent_file_store_does_not_create_home() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let home = root.path().join("missing");
    let storage = create_auth_storage(
        home.clone(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    );
    assert!(!storage.delete()?);
    assert!(!home.try_exists()?);
    Ok(())
}

#[test]
fn write_lock_coordinates_with_existing_auth_lock() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let storage = create_auth_storage(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    );
    let _guard = storage.lock_for_write()?;
    let other_writer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(home.path().join("auth.lock"))?;
    assert!(matches!(
        other_writer.try_lock(),
        Err(TryLockError::WouldBlock)
    ));
    Ok(())
}

#[test]
fn commits_respect_another_process_lock() -> anyhow::Result<()> {
    const CHILD_HOME: &str = "CODEX_AUTH_COMMIT_TEST_HOME";
    if let Some(home) = std::env::var_os(CHILD_HOME) {
        let home = PathBuf::from(home);
        let storage = CommittedStorage {
            inner: Arc::new(FileAuthStorage::new(home.clone())),
            home,
            mode: AuthCredentialsStoreMode::File,
        };
        let _guard = storage.lock()?;
        println!("LOCK_HELD");
        let mut release = String::new();
        std::io::stdin().read_to_string(&mut release)?;
        return Ok(());
    }
    let home = tempfile::tempdir()?;
    let storage = create_auth_storage(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    );
    let old: AuthDotJson = serde_json::from_value(serde_json::json!({"OPENAI_API_KEY":"old"}))?;
    let new: AuthDotJson = serde_json::from_value(serde_json::json!({"OPENAI_API_KEY":"new"}))?;
    storage.save(&old)?;
    let mut child = codex_utils_process::background_command(std::env::current_exe()?)
        .args([
            "--exact",
            "auth::storage::storage_commit::tests::commits_respect_another_process_lock",
            "--nocapture",
        ])
        .env(CHILD_HOME, home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let input = child.stdin.take();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        anyhow::ensure!(
            output.read_line(&mut line)? > 0,
            "lock holder exited before acquiring the lock"
        );
        if line.contains("LOCK_HELD") {
            break;
        }
    }
    let result = storage.compare_and_save(&old, &new);
    drop(input);
    let status = child.wait()?;
    assert!(status.success());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(storage.load()?, Some(old.clone()));
    storage.compare_and_save(&old, &new)?;
    assert_eq!(storage.load()?, Some(new));
    Ok(())
}
