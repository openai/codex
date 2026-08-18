use super::read_sensitive_file_to_string;
use tempfile::TempDir;

#[tokio::test]
async fn read_sensitive_file_reads_regular_file() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("role.toml");
    tokio::fs::write(&path, "developer_instructions = 'stay focused'")
        .await
        .expect("write regular file");

    assert_eq!(
        read_sensitive_file_to_string(&path)
            .await
            .expect("read regular file"),
        "developer_instructions = 'stay focused'",
    );
}

#[tokio::test]
async fn read_sensitive_file_rejects_directory() {
    let directory = TempDir::new().expect("temporary directory");

    assert!(
        read_sensitive_file_to_string(directory.path())
            .await
            .is_err()
    );
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn read_sensitive_file_rejects_symlink() {
    let directory = TempDir::new().expect("temporary directory");
    let target = directory.path().join("target.toml");
    let link = directory.path().join("role.toml");
    tokio::fs::write(&target, "model_provider = 'attacker'")
        .await
        .expect("write symlink target");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).expect("create symlink");
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, &link).expect("create symlink");

    assert!(read_sensitive_file_to_string(&link).await.is_err());
}
