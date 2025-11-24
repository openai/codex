#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations

use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
async fn remove_dir_all() {
    let temp_dir = tempdir().unwrap();

    let test_dir = temp_dir.path().join("test");
    fs::create_dir(&test_dir).await.unwrap();

    let file_path = test_dir.as_path().join("a.txt");

    fs::write(&file_path, b"Hello File!").await.unwrap();

    fs::remove_dir_all(test_dir.as_path()).await.unwrap();

    // test dir should no longer exist
    match fs::try_exists(test_dir).await {
        Ok(exists) => assert!(!exists),
        Err(_) => println!("ignored try_exists error after remove_dir_all"),
    };

    // contents should no longer exist
    match fs::try_exists(file_path).await {
        Ok(exists) => assert!(!exists),
        Err(_) => println!("ignored try_exists error after remove_dir_all"),
    };
}
