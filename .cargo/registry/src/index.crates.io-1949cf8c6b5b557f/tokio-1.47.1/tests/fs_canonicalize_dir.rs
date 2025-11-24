#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations

use tokio::fs;

#[tokio::test]
#[cfg(unix)]
async fn canonicalize_root_dir_unix() {
    assert_eq!(fs::canonicalize("/.").await.unwrap().to_str().unwrap(), "/");
}

#[tokio::test]
#[cfg(windows)]
async fn canonicalize_root_dir_windows() {
    // 2-step let bindings due to Rust memory semantics
    let dir_path = fs::canonicalize("C:\\.\\").await.unwrap();

    let dir_name = dir_path.to_str().unwrap();

    assert!(dir_name.starts_with("\\\\"));
    assert!(dir_name.ends_with("C:\\"));
}
