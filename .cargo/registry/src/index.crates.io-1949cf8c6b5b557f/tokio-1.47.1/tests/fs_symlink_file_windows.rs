#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations
#![cfg(windows)]

use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
async fn symlink_file_windows() {
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("foo.txt");
    let dest_path = dir.path().join("bar.txt");

    fs::write(&source_path, b"Hello File!").await.unwrap();
    fs::symlink_file(&source_path, &dest_path).await.unwrap();

    fs::write(&source_path, b"new data!").await.unwrap();

    let from = fs::read(&source_path).await.unwrap();
    let to = fs::read(&dest_path).await.unwrap();

    assert_eq!(from, to);
}
