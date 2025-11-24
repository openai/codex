#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations

use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `chmod` in miri.
async fn try_exists() {
    let dir = tempdir().unwrap();

    let existing_path = dir.path().join("foo.txt");
    fs::write(&existing_path, b"Hello File!").await.unwrap();
    let nonexisting_path = dir.path().join("bar.txt");

    assert!(fs::try_exists(existing_path).await.unwrap());
    assert!(!fs::try_exists(nonexisting_path).await.unwrap());
    // FreeBSD root user always has permission to stat.
    #[cfg(all(unix, not(target_os = "freebsd")))]
    {
        use std::os::unix::prelude::PermissionsExt;
        let permission_denied_directory_path = dir.path().join("baz");
        fs::create_dir(&permission_denied_directory_path)
            .await
            .unwrap();
        let permission_denied_file_path = permission_denied_directory_path.join("baz.txt");
        fs::write(&permission_denied_file_path, b"Hello File!")
            .await
            .unwrap();
        let mut perms = tokio::fs::metadata(&permission_denied_directory_path)
            .await
            .unwrap()
            .permissions();

        perms.set_mode(0o244);
        fs::set_permissions(&permission_denied_directory_path, perms)
            .await
            .unwrap();
        let permission_denied_result = fs::try_exists(permission_denied_file_path).await;
        assert_eq!(
            permission_denied_result.err().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
}
