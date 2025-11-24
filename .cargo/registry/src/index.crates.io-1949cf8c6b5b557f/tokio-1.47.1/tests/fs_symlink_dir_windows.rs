#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations
#![cfg(windows)]

use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
async fn symlink_file_windows() {
    const FILE_NAME: &str = "abc.txt";

    let temp_dir = tempdir().unwrap();

    let dir1 = temp_dir.path().join("a");
    fs::create_dir(&dir1).await.unwrap();

    let file1 = dir1.as_path().join(FILE_NAME);
    fs::write(&file1, b"Hello File!").await.unwrap();

    let dir2 = temp_dir.path().join("b");
    fs::symlink_dir(&dir1, &dir2).await.unwrap();

    fs::write(&file1, b"new data!").await.unwrap();

    let file2 = dir2.as_path().join(FILE_NAME);

    let from = fs::read(&file1).await.unwrap();
    let to = fs::read(&file2).await.unwrap();

    assert_eq!(from, to);
}
