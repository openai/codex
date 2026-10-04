//! Verifies Windows package publication and selection without requiring an elevated daemon.

use anyhow::Context;
use pretty_assertions::assert_eq;
use std::os::windows::fs::OpenOptionsExt;
use std::time::Duration;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;

#[tokio::test]
async fn publishing_release_retries_transient_access_denied() {
    let home = tempfile::TempDir::new().unwrap();
    let stage = home.path().join(".staging");
    let release = home.path().join("release");
    std::fs::create_dir(&stage).unwrap();
    std::fs::write(stage.join("payload"), b"complete").unwrap();

    let directory = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&stage)
        .unwrap();
    let holder = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        drop(directory);
    });

    super::publish_release(&stage, &release).await.unwrap();
    holder.await.unwrap();
    assert_eq!(std::fs::read(release.join("payload")).unwrap(), b"complete");
}

#[cfg(windows)]
#[test]
fn daemon_junction_can_be_created_retargeted_and_replaced_without_cli_links() {
    let home = tempfile::TempDir::new().unwrap();
    let root = home
        .path()
        .join("home & 100% ready!/packages/app-server-daemon");
    for (version, native_denied) in [("first", true), ("second", false), ("third", true)] {
        let release = root.join("releases").join(version);
        std::fs::create_dir_all(&release).unwrap();
        if native_denied {
            super::select_release_with(&root, &release, |_, _| {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                    .context("failed to retarget managed daemon junction")
            })
            .unwrap();
        } else {
            super::select_release(&root, &release).unwrap();
        }
        assert_eq!(
            root.join("current").canonicalize().unwrap(),
            release.canonicalize().unwrap()
        );
    }
    assert!(!root.with_file_name("standalone").exists());
}
