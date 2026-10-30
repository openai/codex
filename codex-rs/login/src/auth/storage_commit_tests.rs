#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use std::io::BufRead;
use std::io::Read;
use std::process::Stdio;

#[cfg(unix)]
#[test]
fn keyring_commits_ignore_unusable_fallback_symlink_targets() -> anyhow::Result<()> {
    use codex_keyring_store::tests::MockKeyringStore;
    use std::os::unix::fs::symlink;

    for kind in [
        AuthKeyringBackendKind::Direct,
        AuthKeyringBackendKind::Secrets,
    ] {
        for target in ["auth.json", "not-a-directory/auth.json"] {
            let home = tempfile::tempdir()?;
            std::fs::write(home.path().join("not-a-directory"), "")?;
            let auth_file = get_auth_file(home.path());
            let storage = create_auth_storage_with_store(
                home.path().to_path_buf(),
                AuthCredentialsStoreMode::Keyring,
                Arc::new(MockKeyringStore::default()),
                kind,
            );
            let first = serde_json::from_value(serde_json::json!({"OPENAI_API_KEY": "first"}))?;
            let second = serde_json::from_value(serde_json::json!({"OPENAI_API_KEY": "second"}))?;
            let _guard = storage.lock_for_write()?;

            symlink(target, &auth_file)?;
            storage.save(&first)?;
            assert_eq!(storage.load()?, Some(first.clone()));
            assert_eq!(
                std::fs::symlink_metadata(&auth_file).unwrap_err().kind(),
                std::io::ErrorKind::NotFound
            );

            symlink(target, &auth_file)?;
            storage.compare_and_save(&first, &second)?;
            assert_eq!(storage.load()?, Some(second));

            symlink(target, &auth_file)?;
            assert!(storage.delete()?);
            assert_eq!(storage.load()?, None);
            assert_eq!(
                std::fs::symlink_metadata(&auth_file).unwrap_err().kind(),
                std::io::ErrorKind::NotFound
            );
        }
    }
    Ok(())
}

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

#[cfg(unix)]
#[test]
fn keyring_logout_coordinates_with_refresh_through_chained_symlinks() -> anyhow::Result<()> {
    use codex_keyring_store::tests::MockKeyringStore;
    use std::os::unix::fs::symlink;

    const CHILD_HOME: &str = "CODEX_AUTH_CHAINED_REFRESH_TEST_HOME";
    let old: AuthDotJson = serde_json::from_value(serde_json::json!({"OPENAI_API_KEY": "old"}))?;
    let new: AuthDotJson = serde_json::from_value(serde_json::json!({"OPENAI_API_KEY": "new"}))?;
    if let Some(home) = std::env::var_os(CHILD_HOME) {
        let home = PathBuf::from(home);
        let storage = CommittedStorage {
            inner: Arc::new(FileAuthStorage::new(home.clone())),
            home,
            mode: AuthCredentialsStoreMode::File,
        };
        let _write_guard = storage.lock_for_write()?;
        let _commit_guard = storage.lock()?;
        // Pause between validation and save under compare_and_save's commit lock.
        assert_eq!(storage.inner.load_account_grant()?, Some(old));
        println!("REFRESH_VALIDATED");
        let mut release = String::new();
        std::io::stdin().read_to_string(&mut release)?;
        storage.inner.save(&new)?;
        return Ok(());
    }

    for kind in [
        AuthKeyringBackendKind::Direct,
        AuthKeyringBackendKind::Secrets,
    ] {
        let shared = tempfile::tempdir()?;
        let keyring_home = tempfile::tempdir()?;
        let writer_home = tempfile::tempdir()?;
        let shared_storage = FileAuthStorage::new(shared.path().to_path_buf());
        shared_storage.save(&old)?;
        let middle = get_auth_file(keyring_home.path());
        symlink(get_auth_file(shared.path()), &middle)?;
        symlink(&middle, get_auth_file(writer_home.path()))?;
        let storage = create_auth_storage_with_store(
            keyring_home.path().to_path_buf(),
            AuthCredentialsStoreMode::Keyring,
            Arc::new(MockKeyringStore::default()),
            kind,
        );
        let _guard = storage.lock_for_write()?;
        let mut child = codex_utils_process::background_command(std::env::current_exe()?)
            .args([
                "--exact",
                "auth::storage::storage_commit::tests::keyring_logout_coordinates_with_refresh_through_chained_symlinks",
                "--nocapture",
            ])
            .env(CHILD_HOME, writer_home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let input = child.stdin.take();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            anyhow::ensure!(
                output.read_line(&mut line)? > 0,
                "refresh exited before validating credentials"
            );
            if line.contains("REFRESH_VALIDATED") {
                break;
            }
        }
        let result = storage.delete();
        drop(input);
        let status = child.wait()?;
        assert!(status.success());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert!(std::fs::symlink_metadata(&middle)?.file_type().is_symlink());
        assert_eq!(shared_storage.load()?, Some(new.clone()));

        // Logout can unlink the middle symlink once the refresh has committed.
        assert!(storage.delete()?);
        assert_eq!(
            std::fs::symlink_metadata(&middle).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(shared_storage.load()?, Some(new.clone()));
    }
    Ok(())
}
