#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations

use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `fchmod` in miri.
async fn copy() {
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("foo.txt");
    let dest_path = dir.path().join("bar.txt");

    fs::write(&source_path, b"Hello File!").await.unwrap();
    fs::copy(&source_path, &dest_path).await.unwrap();

    let from = fs::read(&source_path).await.unwrap();
    let to = fs::read(&dest_path).await.unwrap();

    assert_eq!(from, to);
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // No `fchmod` in miri.
async fn copy_permissions() {
    let dir = tempdir().unwrap();
    let from_path = dir.path().join("foo.txt");
    let to_path = dir.path().join("bar.txt");

    let from = tokio::fs::File::create(&from_path).await.unwrap();
    let mut from_perms = from.metadata().await.unwrap().permissions();
    from_perms.set_readonly(true);
    from.set_permissions(from_perms.clone()).await.unwrap();

    tokio::fs::copy(from_path, &to_path).await.unwrap();

    let to_perms = tokio::fs::metadata(to_path).await.unwrap().permissions();

    assert_eq!(from_perms, to_perms);
}
