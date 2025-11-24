#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi does not support file operations

use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_test::assert_ok;

#[tokio::test]
async fn fill_buf_file() {
    let file = NamedTempFile::new().unwrap();

    assert_ok!(std::fs::write(file.path(), b"hello"));

    let file = assert_ok!(File::open(file.path()).await);
    let mut file = BufReader::new(file);

    let mut contents = Vec::new();

    loop {
        let consumed = {
            let buffer = assert_ok!(file.fill_buf().await);
            if buffer.is_empty() {
                break;
            }
            contents.extend_from_slice(buffer);
            buffer.len()
        };

        file.consume(consumed);
    }

    assert_eq!(contents, b"hello");
}
