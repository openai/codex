#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi cannot run system commands

use tokio::process::Command;
use tokio_test::assert_ok;

#[tokio::test]
async fn simple() {
    let mut cmd;

    if cfg!(windows) {
        cmd = Command::new("cmd");
        cmd.arg("/c");
    } else {
        cmd = Command::new("sh");
        cmd.arg("-c");
    }

    let mut child = cmd.arg("exit 2").spawn().unwrap();

    let id = child.id().expect("missing id");
    assert!(id > 0);

    let status = assert_ok!(child.wait().await);
    assert_eq!(status.code(), Some(2));

    // test that the `.wait()` method is fused just like the stdlib
    let status = assert_ok!(child.wait().await);
    assert_eq!(status.code(), Some(2));

    // Can't get id after process has exited
    assert_eq!(child.id(), None);
    drop(child.kill());
}
